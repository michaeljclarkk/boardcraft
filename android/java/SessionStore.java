package dev.boardcraft.kitfixture;

import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.io.DataInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.GeneralSecurityException;
import java.security.KeyStore;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/** JNI calls run on a worker. The only supported origin is HTTP IPv4 loopback. */
public final class SessionStore {
    private static final int VERSION = 1, IV_BYTES = 12, TOKEN_BYTES = 43, TAG_BYTES = 16;
    private static final String ALIAS = "boardcraft.session.aes";

    private static SecretKey key(boolean create) throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        SecretKey key = (SecretKey) store.getKey(ALIAS, null);
        if (key != null) return key;
        if (!create) throw new GeneralSecurityException("Missing session key");
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        generator.init(new KeyGenParameterSpec.Builder(ALIAS,
                KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).build());
        return generator.generateKey();
    }

    private static Path record(Context app, int port) throws IOException {
        if (port <= 0 || port > 65535) throw new IOException("Invalid port");
        Path dir = app.getNoBackupFilesDir().toPath();
        if (!Files.isDirectory(dir)) throw new IOException("No session directory");
        return dir.resolve("http-127.0.0.1-" + port);
    }

    private static void authenticate(Cipher cipher, int port) {
        cipher.updateAAD((VERSION + " http://127.0.0.1:" + port).getBytes(StandardCharsets.US_ASCII));
    }

    private static Path pending(Path file) {
        return file.resolveSibling(file.getFileName() + ".pending");
    }

    public static synchronized void save(Context app, int port, String token) throws Exception {
        Path file = record(app, port);
        if (!token.matches("[A-Za-z0-9_-]{43}")) throw new IOException("Invalid session");
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, key(true)); // Keystore chooses a fresh IV
        authenticate(cipher, port);
        byte[] iv = cipher.getIV();
        byte[] sealed = cipher.doFinal(token.getBytes(StandardCharsets.US_ASCII));
        if (iv.length != IV_BYTES || sealed.length != TOKEN_BYTES + TAG_BYTES)
            throw new GeneralSecurityException("Unexpected GCM parameters");
        Path tmp = pending(file);
        try {
            try (FileOutputStream out = new FileOutputStream(tmp.toFile())) {
                out.write(VERSION);
                out.write(iv);
                out.write(sealed);
                out.getFD().sync();
            }
            Files.move(tmp, file, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } finally {
            Files.deleteIfExists(tmp);
        }
    }

    public static synchronized String load(Context app, int port) throws Exception {
        Path file = record(app, port);
        byte[] iv = new byte[IV_BYTES], sealed = new byte[TOKEN_BYTES + TAG_BYTES];
        try (DataInputStream in = new DataInputStream(Files.newInputStream(file))) {
            if (in.read() != VERSION) throw new IOException("Unknown session format");
            in.readFully(iv);
            in.readFully(sealed);
            if (in.read() != -1) throw new IOException("Invalid session length");
        } catch (NoSuchFileException missing) {
            return null;
        }
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.DECRYPT_MODE, key(false), new GCMParameterSpec(TAG_BYTES * 8, iv));
        authenticate(cipher, port);
        byte[] plain = cipher.doFinal(sealed);
        try {
            String token = new String(plain, StandardCharsets.US_ASCII);
            if (!token.matches("[A-Za-z0-9_-]{43}")) throw new IOException("Invalid session");
            return token;
        } finally {
            Arrays.fill(plain, (byte) 0);
        }
    }

    public static synchronized void forget(Context app, int port) throws IOException {
        Path file = record(app, port);
        Files.deleteIfExists(file);
        Files.deleteIfExists(pending(file));
    }
}
