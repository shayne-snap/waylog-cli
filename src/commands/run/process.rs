use tokio::process::Child;
use tokio::time::{timeout, Duration};
use tracing;

/// Terminate child process with timeout
pub(crate) async fn terminate_child(child: &mut Child) {
    // Try to kill the child process
    if let Err(e) = child.kill().await {
        // Child might have already exited
        tracing::debug!(
            "Failed to kill child process (may have already exited): {}",
            e
        );
        return;
    }

    // Wait for child to exit with timeout (5 seconds)
    match timeout(Duration::from_secs(5), child.wait()).await {
        Ok(Ok(_)) => {
            tracing::debug!("Child process terminated successfully");
        }
        Ok(Err(e)) => {
            tracing::warn!("Error waiting for child process: {}", e);
        }
        Err(_) => {
            tracing::warn!("Timeout waiting for child process to terminate, continuing cleanup");
        }
    }
}
