mod exporter;
mod health;

use crate::exporter::TapoClient;
use clap::{Command, CommandFactory, Parser, Subcommand};
use clap_complete::aot::{Generator, Shell, generate};
use std::{error, io};
use tapo::{ApiClient, Error};

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

        /// IP address or DNS name for the devices
        #[arg(
            short,
            long,
            env = "IP_ADDRESS",
            hide_env_values = true,
            value_delimiter = ' '
        )]
        device_addresses: Vec<String>,
    },
    /// Generate shell auto-completions
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let cli = Cli::parse();

    let port = cli.port;

    match &cli.command {
        Some(Commands::Health {}) => Ok(health::health(port).await?),
        Some(Commands::Server {
            username,
            password,
            device_addresses,
        }) => {
            let mut clients: Vec<Box<dyn TapoClient + Send + Sync>> = Vec::new();

            for device_address in device_addresses {
                let client = client_for_device(username, password, device_address).await?;

                clients.push(client);
            }

            let router = exporter::app(clients);

            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;

            println!("Server is listening on {port}");
            Ok(axum::serve(listener, router).await?)
        }
        Some(Commands::Completion { shell }) => {
            let mut cmd = Cli::command();
            print_completions(*shell, &mut cmd);
            Ok(())
        }
        None => {
            panic!("No command provided");
        }
    }
}

async fn client_for_device(
    username: &str,
    password: &str,
    device_address: &str,
) -> Result<Box<dyn TapoClient + Send + Sync>, Error> {
    let client = ApiClient::new(username, password);

    let p304client = client.clone().p304(device_address).await?;

    // There's nothing in the API to peak at the device to find its type
    // so just pick any one and check the model
    match p304client.get_device_info().await?.model.as_str() {
        "P110M" => Ok(Box::new(exporter::PlugClient {
            client: client.p110(device_address).await?,
        })),
        "P304M" => Ok(Box::new(exporter::PowerStripClient { client: p304client })),
        model => panic!("Unknown model: {}", model),
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
