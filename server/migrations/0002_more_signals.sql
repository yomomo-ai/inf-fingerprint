-- Persistence super-cookie. The single most decisive signal: when this matches
-- a stored signature AND that signature otherwise scores ≥ ambiguous_threshold
-- against incoming features, we short-circuit the bucket scan.
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS client_visitor_id TEXT;
CREATE INDEX IF NOT EXISTS idx_sig_client_visitor_id
    ON signatures(client_visitor_id) WHERE client_visitor_id IS NOT NULL;

-- Battery — Chromium-only (X5/XWEB exposes; iOS Safari does not). Absence is
-- itself a signal.
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS battery_charging BOOLEAN;
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS battery_level DOUBLE PRECISION;

-- StorageManager.estimate() — quota is fairly stable per device.
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS storage_quota_bytes BIGINT;
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS storage_usage_bytes BIGINT;

-- UA Client Hints high-entropy values (Chromium async API).
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS ua_architecture TEXT;
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS ua_bitness TEXT;
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS ua_model TEXT;
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS ua_platform_version TEXT;
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS ua_full_version TEXT;

-- WebRTC public IPs (via STUN). Multiple if dual-stack or multiple NICs.
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS webrtc_public_ips TEXT[];
ALTER TABLE signatures ADD COLUMN IF NOT EXISTS webrtc_local_ips TEXT[];

-- Request-context attributes (server-derived from HTTP, for forensics).
ALTER TABLE observations ADD COLUMN IF NOT EXISTS request_user_agent TEXT;
ALTER TABLE observations ADD COLUMN IF NOT EXISTS request_accept_language TEXT;
ALTER TABLE observations ADD COLUMN IF NOT EXISTS request_sec_ch_ua TEXT;
ALTER TABLE observations ADD COLUMN IF NOT EXISTS request_sec_ch_ua_platform TEXT;
ALTER TABLE observations ADD COLUMN IF NOT EXISTS request_referer TEXT;
ALTER TABLE observations ADD COLUMN IF NOT EXISTS request_dnt BOOLEAN;
