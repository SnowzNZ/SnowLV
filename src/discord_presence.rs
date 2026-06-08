//! Discord Rich Presence integration.
//!
//! The integration is intentionally best-effort: SnowLV should launch and run
//! normally when Discord is closed, unavailable, or not configured.

use discord_rich_presence::{
    activity::{self, ActivityType},
    DiscordIpc, DiscordIpcClient,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Set this to SnowLV's Discord Developer Portal application ID for release
/// builds, or set `SNOWLV_DISCORD_CLIENT_ID` in the environment while testing.
const DEFAULT_DISCORD_CLIENT_ID: &str = "1511704156736327871";
const GITHUB_REPOSITORY_URL: &str = "https://github.com/SnowzNZ/SnowLV";
const RECONNECT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct DiscordPresence {
    client_id: Option<String>,
    client: Option<DiscordIpcClient>,
    start_timestamp_ms: i64,
    last_connect_attempt: Option<Instant>,
    last_activity_signature: Option<ActivitySignature>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ActivitySignature {
    pub details: String,
    pub state: String,
}

impl ActivitySignature {
    pub fn new(details: impl Into<String>, state: impl Into<String>) -> Self {
        Self {
            details: details.into(),
            state: state.into(),
        }
    }
}

impl DiscordPresence {
    pub fn new() -> Self {
        let client_id = configured_client_id();
        if client_id.is_none() {
            tracing::debug!(
                "Discord Rich Presence disabled; no SnowLV Discord application ID configured"
            );
        }

        Self {
            client_id,
            start_timestamp_ms: current_unix_timestamp_ms(),
            ..Self::default()
        }
    }

    pub fn update(&mut self, signature: ActivitySignature) {
        if self.last_activity_signature.as_ref() == Some(&signature) {
            return;
        }

        if self.ensure_connected().is_err() {
            return;
        }

        let activity = activity::Activity::new()
            .activity_type(ActivityType::Playing)
            .details(signature.details.clone())
            .state(signature.state.clone())
            .buttons(vec![activity::Button::new(
                "View on GitHub",
                GITHUB_REPOSITORY_URL,
            )])
            .timestamps(activity::Timestamps::new().start(self.start_timestamp_ms));

        let Some(client) = self.client.as_mut() else {
            return;
        };

        match client.set_activity(activity) {
            Ok(()) => {
                self.last_activity_signature = Some(signature);
            }
            Err(e) => {
                tracing::debug!("Failed to update Discord Rich Presence: {}", e);
                self.client = None;
                self.last_activity_signature = None;
                self.last_connect_attempt = Some(Instant::now());
            }
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.clear_activity();
            let _ = client.close();
        }
        self.last_activity_signature = None;
    }

    fn ensure_connected(&mut self) -> Result<(), ()> {
        if self.client.is_some() {
            return Ok(());
        }

        let Some(client_id) = &self.client_id else {
            return Err(());
        };

        if let Some(last_attempt) = self.last_connect_attempt {
            if last_attempt.elapsed() < RECONNECT_INTERVAL {
                return Err(());
            }
        }
        self.last_connect_attempt = Some(Instant::now());

        let mut client = DiscordIpcClient::new(client_id);
        match client.connect() {
            Ok(()) => {
                tracing::info!("Discord Rich Presence connected");
                self.client = Some(client);
                Ok(())
            }
            Err(e) => {
                tracing::debug!("Discord Rich Presence unavailable: {}", e);
                Err(())
            }
        }
    }
}

fn configured_client_id() -> Option<String> {
    std::env::var("SNOWLV_DISCORD_CLIENT_ID")
        .ok()
        .filter(|id| is_valid_client_id(id))
        .or_else(|| {
            let id = DEFAULT_DISCORD_CLIENT_ID.trim();
            is_valid_client_id(id).then(|| id.to_string())
        })
}

fn is_valid_client_id(id: &str) -> bool {
    let trimmed = id.trim();
    (17..=20).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_digit())
}

fn current_unix_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}
