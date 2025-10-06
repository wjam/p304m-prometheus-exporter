mod exporter;
mod health;

use clap::{Command, CommandFactory, Parser, Subcommand};
use clap_complete::aot::{Generator, Shell, generate};
use std::io;
use tapo::ApiClient;

#[derive(Parser)]
#[command(arg_required_else_help = true, version = option_env!("VERSION").unwrap_or("dev-build"))]
struct Cli {
    /// Port number the server is or should be running on
    #[arg(short, long, env, default_value_t = 8080)]
    port: u16,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Perform health check against server as Docker health check doesn't support simple HTTP endpoints
    Health {},
    /// Run server
    Server {
        /// Username for the Tapo service
        #[arg(short, long, env = "TAPO_USERNAME", hide_env_values = true)]
        username: String,

        /// Password for the Tapo service
        #[arg(short, long, env = "TAPO_PASSWORD", hide_env_values = true)]
        password: String,

        /// IP address or DNS name for the P304M device
        #[arg(short, long, env = "IP_ADDRESS", hide_env_values = true)]
        device_address: String,
    },
    /// Generate shell auto-completions
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let port = cli.port;

    match &cli.command {
        Some(Commands::Health {}) => {
            health::health(port).await.unwrap();
        }
        Some(Commands::Server {
            username,
            password,
            device_address,
        }) => {
            let power_strip = ApiClient::new(username, password)
                .p304(device_address)
                .await
                .unwrap();

            let router = exporter::app(exporter::Client {
                _client: Some(power_strip),
            });

            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
                .await
                .unwrap();

            println!("Server is listening on {port}");
            axum::serve(listener, router).await.unwrap();
        }
        Some(Commands::Completion { shell }) => {
            let mut cmd = Cli::command();
            print_completions(*shell, &mut cmd);
        }
        None => {
            panic!("No command provided");
        }
    }
}

fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}
