DO $$ BEGIN
    IF NOT EXISTS (SELECT FROM pg_constraint WHERE conrelid='accounts'::regclass AND conname='accounts_supported_role') THEN
        ALTER TABLE accounts DROP CONSTRAINT accounts_role_check,
            ADD CONSTRAINT accounts_supported_role CHECK (role IN ('viewer','member','owner'));
    END IF;
END $$;
