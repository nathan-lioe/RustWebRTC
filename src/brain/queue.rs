use anyhow::Result;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct QueueState {
    pub position: u32,
    pub estimated_wait_time: Duration,
}

#[derive(Debug)]
pub struct SessionState {
    pub user_id: String,
    pub start_time: Instant,
    pub last_activity: Instant,
}

pub struct Queue {
    waiting: VecDeque<String>,
    active_session: Option<SessionState>,
    state_tx: broadcast::Sender<QueueState>,
    max_session_duration: Duration,
    max_idle_time: Duration,
}

impl Queue {
    pub fn new() -> Self {
        let (state_tx, _) = broadcast::channel(100);

        Self {
            waiting: VecDeque::new(),
            active_session: None,
            state_tx,
            max_session_duration: crate::brain::MAX_SESSION_DURATION,
            max_idle_time: Duration::from_secs(60),
        }
    }

    pub async fn join_queue(&mut self, user_id: String) -> Result<QueueState> {
        if self.waiting.contains(&user_id) {
            return Err(anyhow::anyhow!("User already in queue"));
        }

        self.waiting.push_back(user_id);
        let state = self.get_queue_state();
        self.broadcast_state(&state).await;

        Ok(state)
    }

    pub async fn leave_queue(&mut self, user_id: &str) -> Result<()> {
        self.waiting.retain(|id| id != user_id);
        self.broadcast_state(&self.get_queue_state()).await;
        Ok(())
    }

    pub fn has_pending_users(&self) -> bool {
        !self.waiting.is_empty() || self.active_session.is_some()
    }

    pub async fn update_activity(&mut self, user_id: &str) -> Result<()> {
        if let Some(session) = &mut self.active_session {
            if session.user_id == user_id {
                session.last_activity = Instant::now();
                return Ok(());
            }
        }
        Err(anyhow::anyhow!("No active session for user"))
    }

    pub async fn cleanup_stale_sessions(&mut self) -> Result<()> {
        if let Some(session) = &self.active_session {
            let now = Instant::now();
            if now.duration_since(session.start_time) > self.max_session_duration
                || now.duration_since(session.last_activity) > self.max_idle_time
            {
                self.end_session().await?;
            }
        }
        Ok(())
    }

    async fn end_session(&mut self) -> Result<()> {
        self.active_session = None;
        self.process_queue().await?;
        Ok(())
    }

    async fn process_queue(&mut self) -> Result<()> {
        if self.active_session.is_none() && !self.waiting.is_empty() {
            if let Some(next_user) = self.waiting.pop_front() {
                self.active_session = Some(SessionState {
                    user_id: next_user,
                    start_time: Instant::now(),
                    last_activity: Instant::now(),
                });
                self.broadcast_state(&self.get_queue_state()).await;
            }
        }
        Ok(())
    }

    fn get_queue_state(&self) -> QueueState {
        let position = self.waiting.len() as u32;
        let estimated_wait_time = Duration::from_secs(position as u64 * 600); // Rough estimate

        QueueState {
            position,
            estimated_wait_time,
        }
    }

    async fn broadcast_state(&self, state: &QueueState) {
        let _ = self.state_tx.send(state.clone());
    }

    pub fn subscribe_to_updates(&self) -> broadcast::Receiver<QueueState> {
        self.state_tx.subscribe()
    }
}
