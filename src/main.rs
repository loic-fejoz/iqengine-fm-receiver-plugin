#[macro_use]
extern crate serde_derive;
extern crate axum;

use axum::{
    debug_handler,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    routing::{get, options, post},
    Json, Router,
};
use iqengine_plugin::server::{
    FileJobStorage, FunctionParameters, FunctionPostRequest, FunctionPostResponse, IQFunction, IQFunction1, JobResultResponse, JobStatus, JobStatusResponse, JobStorage
};
use simple_logger::SimpleLogger;
use uuid::Uuid;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

mod fm_receiver;
use fm_receiver::FmReceiverParams;
use fm_receiver::FM_RECEIVER_FUNCTION;

mod amplifier;
use amplifier::AmplifierParams;
use amplifier::AMPLIFIER_FUNCTION;

#[derive(Clone)]
struct AppState {
    pub job_storage: Arc<Mutex<FileJobStorage>>,
}

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

    let job_storage = FileJobStorage::new();
    let state = AppState {
        job_storage: Arc::new(Mutex::new(job_storage)),
    };

    // build our application with a route
    let app = Router::new()
        .route("/plugins", get(get_functions_list))
        .route("/plugins/", get(get_functions_list))
        .route("/plugins/:functionname", options(options_function))
        .route("/plugins/fm-receiver", get(get_fm_receiver_params))
        .route("/plugins/fm-receiver", post(post_fm_receiver))
        .route("/plugins/amplifier", get(get_amplifier_params))
        .route("/plugins/amplifier", post(post_amplifier))
        .route("/plugins/:job_id/status", get(get_job_status))
        .route("/plugins/:job_id/result", get(get_job_result))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(cors))
        .layer(DefaultBodyLimit::disable());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn options_function() -> (StatusCode, Json<String>) {
    (StatusCode::OK, Json("preflight ok".to_string()))
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
    State(state): State<AppState>,
    Json(req): Json<FunctionPostRequest<FmReceiverParams>>,
) -> (StatusCode, Json<FunctionPostResponse>) {
    let res = FM_RECEIVER_FUNCTION.apply(req).await;
    if let Ok(res) = res {
        return (StatusCode::OK, Json(res));
    }
    let mut resp = FunctionPostResponse::new();
    let details = res.unwrap_err().to_string();
    resp.details = Some(details);
    (StatusCode::BAD_REQUEST, Json(resp))
}

// Apply the amplifier
#[debug_handler]
async fn post_amplifier(
    State(state): State<AppState>,
    Json(req): Json<FunctionPostRequest<AmplifierParams>>,
) -> (StatusCode, Json<FunctionPostResponse>) {
    let res = AMPLIFIER_FUNCTION.apply(req).await;
    if let Ok(res) = res {
        return (StatusCode::OK, Json(res));
    }
    let mut resp = FunctionPostResponse::new();
    let details = res.unwrap_err().to_string();
    resp.details = Some(details);
    (StatusCode::BAD_REQUEST, Json(resp))
}

// Return status of job
async fn get_job_status(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> (StatusCode, Json<JobStatusResponse<uuid::Uuid>>) {
    let storage = state.job_storage.lock().expect("msg");
    let Ok(status) = storage.job_status(job_id) else {
        return (StatusCode::NOT_FOUND, Json(JobStatusResponse::not_found(job_id)))
    };
    (StatusCode::OK, Json(status.into()))
}

// Return result of job
async fn get_job_result(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> (StatusCode, Json<JobResultResponse<uuid::Uuid>>) {
    let storage = state.job_storage.lock().expect("msg");
    let Ok(job_result) = storage.job_result(job_id) else {
        return (StatusCode::NOT_FOUND, Json(JobResultResponse::not_found(job_id)))
    };
    if job_result.job_status.progress < 100.0 || job_result.job_status.error.is_some()  {
        return (StatusCode::BAD_REQUEST, Json(job_result.into()))
    }
    (StatusCode::OK, Json(job_result.into()))
}