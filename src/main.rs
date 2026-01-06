use std::sync::Arc;
use actix_web::{App, HttpServer, middleware};
use iqengine_plugin::server::{
    Orchestrator, JobStore, PluginServer, configure_plugin
};
use simple_logger::SimpleLogger;

mod fm_receiver;
use fm_receiver::FM_RECEIVER_FUNCTION;

mod amplifier;
use amplifier::AMPLIFIER_FUNCTION;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    SimpleLogger::new().init().unwrap();

    let job_store = Arc::new(JobStore::new("jobs").unwrap());
    let orchestrator = Arc::new(Orchestrator::new(job_store));

    let host = "127.0.0.1";
    let port = 8000;
    println!("listening on {}:{}", host, port);

    let mut plugin_server = PluginServer::new(orchestrator.clone());
    plugin_server.add_plugin::<fm_receiver::FmReceiverFunction, fm_receiver::FmReceiverParams>("fm-receiver");
    plugin_server.add_plugin::<amplifier::AmplifierFunction, amplifier::AmplifierParams>("amplifier");

    HttpServer::new(move || {
        let ps = plugin_server.clone();
        App::new()
            .wrap(middleware::Logger::default())
            .configure(|cfg| ps.configure(cfg))
            .configure(|cfg| {
                configure_plugin::<_, fm_receiver::FmReceiverParams>(cfg, "fm-receiver", Arc::new(FM_RECEIVER_FUNCTION));
                configure_plugin::<_, amplifier::AmplifierParams>(cfg, "amplifier", Arc::new(AMPLIFIER_FUNCTION));
            })
    })
    .bind((host, port))?
    .run()
    .await
}
