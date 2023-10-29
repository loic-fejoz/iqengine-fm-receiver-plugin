#[macro_use]
extern crate serde_derive;
extern crate axum;

use axum::{
    debug_handler,
    extract::DefaultBodyLimit,
    http::StatusCode,
    routing::{get, post, options},
    Json, Router,
};
use http::Method;
use iqengine_plugin::server::{
    FunctionParameters, FunctionPostRequest, FunctionPostResponse, IQFunction,
};
use simple_logger::SimpleLogger;
use std::{net::SocketAddr, collections::HashMap};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer, Cors};

mod fm_receiver;
use fm_receiver::FmReceiverParams;
use fm_receiver::FM_RECEIVER_FUNCTION;

mod amplifier;
use amplifier::AmplifierParams;
use amplifier::AMPLIFIER_FUNCTION;

#[tokio::main]
async fn main() {
    SimpleLogger::new().init().unwrap();

    // initialize tracing
    //tracing_subscriber::fmt::init();

    // let cors = CorsLayer::new()
    //     .allow_origin(Any)
    //     .allow_headers(Any)
    //     // .allow_credentials(true)
    //     .allow_methods(vec![Method::GET, Method::POST]);
    let cors = CorsLayer::very_permissive();

    // build our application with a route
    let app = Router::new()
        .route("/plugins", get(get_functions_list))
        .route("/plugins/", get(get_functions_list))
        .route("/plugins/:functionname", options(options_function))
        .route("/plugins/fm-receiver", get(get_fm_receiver_params))
        .route("/plugins/fm-receiver", post(post_fm_receiver))
        .route("/plugins/amplifier", get(get_amplifier_params))
        .route("/plugins/amplifier", post(post_amplifier))
        .layer(ServiceBuilder::new().layer(cors))
        .layer(DefaultBodyLimit::disable());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// Return list of IQEngine functions
async fn get_functions_list() -> (StatusCode, Json<Vec<&'static str>>) {
    let functions_list = vec!["fm-receiver", "amplifier"];
    (StatusCode::OK, Json(functions_list))
}

// Describe the parameters for the fm-receiver
async fn get_fm_receiver_params() -> (StatusCode, Json<FunctionParameters>) {
    let custom_params = FM_RECEIVER_FUNCTION.parameters();
    (StatusCode::OK, Json(custom_params))
}

// Describe the parameters for the fm-receiver
async fn get_amplifier_params() -> (StatusCode, Json<FunctionParameters>) {
    let custom_params = AMPLIFIER_FUNCTION.parameters();
    (StatusCode::OK, Json(custom_params))
}

// Apply the fm-receiver
#[debug_handler]
async fn post_fm_receiver(
    Json(req): Json<FunctionPostRequest<FmReceiverParams>>,
) -> (StatusCode, Json<FunctionPostResponse>) {
    let res = FM_RECEIVER_FUNCTION.apply(req);
    if let Ok(res) = res {
        return (StatusCode::OK, Json(res));
    }
    let mut resp = FunctionPostResponse::new();
    let details = res.unwrap_err().to_string();
    resp.details = Some(details);
    return (StatusCode::BAD_REQUEST, Json(resp));
}

// Apply the amplifier
#[debug_handler]
async fn post_amplifier(
    Json(req): Json<FunctionPostRequest<AmplifierParams>>,
) -> (StatusCode, Json<FunctionPostResponse>) {
    let res = AMPLIFIER_FUNCTION.apply(req);
    if let Ok(res) = res {
        return (StatusCode::OK, Json(res));
    }
    let mut resp = FunctionPostResponse::new();
    let details = res.unwrap_err().to_string();
    resp.details = Some(details);
    return (StatusCode::BAD_REQUEST, Json(resp));
}
