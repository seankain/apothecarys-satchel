mod error;
mod server;
mod state;
mod tools;

use server::FyroxMcpServer;
use state::ServerState;

use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

fn detect_project_root() -> std::path::PathBuf {
    if let Ok(root) = std::env::var("APOTHECARYS_ROOT") {
        return std::path::PathBuf::from(root);
    }

    let mut dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return dir;
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }

    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let project_root = detect_project_root();
    tracing::info!("Project root: {}", project_root.display());

    let state = ServerState::new(project_root)?;
    let server = FyroxMcpServer::new(state);

    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
