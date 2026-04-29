-- search_path is set to <schema>,public on every pooled connection (see store.rs),
-- so unqualified CREATE TABLE statements land in our schema.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS visitors (
    visitor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    observation_count BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS signatures (
    visitor_id UUID PRIMARY KEY REFERENCES visitors(visitor_id) ON DELETE CASCADE,
    bucket_key BYTEA NOT NULL,

    canonical_ua_hash TEXT NOT NULL,
    math_fp_hash TEXT,
    webgl_params_hash TEXT,
    webgl_render_hash TEXT,
    canvas_hash TEXT,
    audio_hash TEXT,
    audio_stable_checksum DOUBLE PRECISION,
    speech_voices_hash TEXT,
    fonts_sorted_hash TEXT,
    dom_rect_hash TEXT,

    screen_w INT,
    screen_h INT,
    device_pixel_ratio DOUBLE PRECISION,
    color_depth INT,
    hw_concurrency DOUBLE PRECISION,
    device_memory DOUBLE PRECISION,
    max_touch_points INT,

    timezone TEXT,
    locale TEXT,
    language_tag TEXT,

    in_app TEXT,
    in_app_version TEXT,
    in_app_version_code TEXT,
    wechat_platform TEXT,
    device_vendor TEXT,
    system_rom TEXT,
    system_version TEXT,
    device_model TEXT,
    android_build TEXT,

    ua_consistent BOOLEAN,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_sig_bucket ON signatures(bucket_key);
CREATE INDEX IF NOT EXISTS idx_sig_canonical_ua ON signatures(canonical_ua_hash);
CREATE INDEX IF NOT EXISTS idx_sig_math_fp ON signatures(math_fp_hash) WHERE math_fp_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sig_webgl_render ON signatures(webgl_render_hash) WHERE webgl_render_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS observations (
    observation_id BIGSERIAL PRIMARY KEY,
    visitor_id UUID NOT NULL REFERENCES visitors(visitor_id) ON DELETE CASCADE,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    bucket_key BYTEA NOT NULL,
    match_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    match_kind TEXT NOT NULL,
    ip_address INET,
    user_agent TEXT,
    raw_features JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_obs_visitor ON observations(visitor_id);
CREATE INDEX IF NOT EXISTS idx_obs_observed_at ON observations(observed_at DESC);
CREATE INDEX IF NOT EXISTS idx_obs_match_kind ON observations(match_kind);
