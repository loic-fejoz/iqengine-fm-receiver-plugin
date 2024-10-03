#[macro_use]
extern crate serde_derive;
extern crate axum;

use axum::{
    debug_handler,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::StatusCode,
    routing::{get, options, post},
    Json, Router,
};
use iqengine_plugin::server::{
    FileJobStorage, FunctionParameters, FunctionRequest1, FunctionRequest1Builder, IQEngineError,
    IQFunction1, JobResultResponse, JobStatus, JobStatusResponse, JobStorage,
};
use serde::Serialize;
use simple_logger::SimpleLogger;
use std::{
    env,
    sync::{Arc, Mutex},
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

mod fm_receiver;
use fm_receiver::FM_RECEIVER_FUNCTION;
use fm_receiver::{FmReceiverFunction, FmReceiverParams};

mod amplifier;
use amplifier::AMPLIFIER_FUNCTION;
use amplifier::{AmplifierFunction, AmplifierParams};

#[derive(Clone)]
struct AppState {
    pub job_storage: Arc<Mutex<FileJobStorage>>,
}

impl AppState {
    pub fn add_task<T, F, I>(
        &self,
        func: F,
        req: FunctionRequest1<T>,
    ) -> Result<JobStatus<Uuid>, IQEngineError>
    where
        F: iqengine_plugin::server::IQFunction1<T> + Send + 'static,
        I: ToString + Send,
        T: Serialize + Send + Clone + 'static,
    {
        let id = Uuid::new_v4();
        let job_status = self.job_storage.lock().unwrap().new_status(id)?;
        let status = job_status.clone();
        let storage = self.job_storage.clone();
        tokio::spawn(async move {
            let result = func.apply(req, status).await;
            if let Ok(mut result) = result {
                result.job_status.progress = 100.0;
                result.job_status.error = None;
                let _ = storage.lock().unwrap().store_result(id, result);
            } else {
                let _ = storage.lock().unwrap().set_status(
                    id,
                    100.0,
                    Some(result.err().expect("msg").to_string()),
                );
            };
        });
        Ok(job_status)
    }
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
    let _ = job_storage.init();
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
        .layer(ServiceBuilder::new().layer(cors))
        .with_state(state)
        .layer(DefaultBodyLimit::disable())
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)); // 20Mb of limit

    let addr = match env::var("IQENGINE_IP") {
        std::result::Result::Ok(v) => v,
        Err(_e) => "127.0.0.1".to_string(),
    };
    let port = match env::var("IQENGINE_PORT") {
        std::result::Result::Ok(v) => v,
        Err(_e) => "8000".to_string(),
    };
    let mut addr = addr.to_owned();
    addr.push_str(":");
    addr.push_str(&port);
    let addr = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!(
        "Listening on {}...\nYou can know open https://www.iqengine.org",
        port
    );
    axum::serve(addr, app).await.expect("msg");
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
    multipart: Multipart,
) -> (StatusCode, Json<JobStatus<Uuid>>) {
    let req = FunctionRequest1Builder::<FmReceiverParams>::parse(multipart).await;
    if let Err(ret) = req {
        return ret;
    }
    let req = req.expect("valid at this point");

    let job_status =
        state.add_task::<FmReceiverParams, FmReceiverFunction, Uuid>(FM_RECEIVER_FUNCTION, req);
    if let Ok(job_status) = job_status {
        (StatusCode::OK, Json(job_status))
    } else {
        let resp = JobStatus::<Uuid>::error(job_status.err().unwrap());
        (StatusCode::BAD_REQUEST, Json(resp))
    }
}

// Apply the amplifier
#[debug_handler]
async fn post_amplifier(
    State(state): State<AppState>,
    multipart: Multipart,
) -> (StatusCode, Json<JobStatus<Uuid>>) {
    let req = FunctionRequest1Builder::<AmplifierParams>::parse(multipart).await;
    if let Err(ret) = req {
        return ret;
    }
    let req = req.expect("valid at this point");

    let job_status =
        state.add_task::<AmplifierParams, AmplifierFunction, Uuid>(AMPLIFIER_FUNCTION, req);
    if let Ok(job_status) = job_status {
        (StatusCode::OK, Json(job_status))
    } else {
        let resp = JobStatus::<Uuid>::error(job_status.err().unwrap());
        (StatusCode::BAD_REQUEST, Json(resp))
    }
}

// Return status of job
async fn get_job_status(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> (StatusCode, Json<JobStatusResponse<uuid::Uuid>>) {
    let storage = state.job_storage.lock().expect("msg");
    let Ok(status) = storage.job_status(job_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(JobStatusResponse::not_found(job_id)),
        );
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
        return (
            StatusCode::NOT_FOUND,
            Json(JobResultResponse::not_found(job_id)),
        );
    };
    if job_result.job_status.progress < 100.0 || job_result.job_status.error.is_some() {
        return (StatusCode::BAD_REQUEST, Json(job_result.into()));
    }
    (StatusCode::OK, Json(job_result.into()))
}
