# Browser input compatibility

`cargo run --locked --manifest-path tools/Cargo.toml -- prepare-web` extracts the checksum-verified public `gpui-pre-web`
0.3.14 crate into ignored `.deps/`. GPUI core is the unpatched, registry-locked
`gpui-pre` 0.3.14 release. Native accessibility and ordinary Kit semantics are
unchanged. There is no browser accessibility adapter, semantic DOM tree, action
forwarder or hidden copy of the canvas controls.

Stock web input mirrors the focused handler's raw text into a textarea, including
masked Kit input. The input-only patch substitutes **one permanently protected
password receiver**, retaining upstream keyboard, selection, composition and IME
dispatch. It never selects protection by reading accessibility or application
state. Password text is masked by Kit on the canvas and by the browser at the
IME/accessibility boundary. The normal same-origin host still owns the bearer.

All current canvas fields are single-line. This deliberately small patch is not
a general multiline-input solution: a browser password input strips newlines.
Revisit this platform boundary before introducing multiline controls. Software
keyboards, physical IMEs and mobile browsers have not been accepted here.

After a frame, upstream's coalesced input sync also reconciles removed editors,
so async navigation and logout do not leave their text in the receiver. It does
not add another input event or duplicate command dispatch.

Use the [ordinary browser viewer](../../../docs/development.md#browser-viewer)
to try the current build. Use Computer Use for live observations and GPUI Kit
tests for control behavior. Masked text on the canvas alone does not establish
that the browser's IME protects a password.
