use base64::Engine;
use iqengine_plugin::server::{
    error::IQEngineError, CustomParamType, FunctionParameters, FunctionParamsBuilder,
    FunctionPostRequest,
};
use log::debug;
use num_complex::Complex32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifierParams {
    #[serde(rename = "a")]
    a: f32,
}

pub struct AmplifierFunction {}

impl iqengine_plugin::server::IQFunction<AmplifierParams> for AmplifierFunction {
    fn parameters(&self) -> FunctionParameters {
        FunctionParamsBuilder::new()
            .max_inputs(1)
            .max_outputs(1)
            .custom_param(
                "a",
                "amplitude multiplier coefficient",
                CustomParamType::Number,
                Some("1.0"),
            )
            .build()
    }

    async fn apply(
        &self,
        request: FunctionPostRequest<AmplifierParams>,
        samples: Vec<num_complex::Complex32>,
        job_id: String,
        job_store: std::sync::Arc<iqengine_plugin::server::JobStore>,
    ) -> Result<iqengine_plugin::server::Output, IQEngineError> {
        debug!("Applying amplifier for job {}...", job_id);

        let a = if let Some(prop) = request.custom_params {
            prop.a
        } else {
            return Err(IQEngineError::MandatoryParameter("a".to_string()));
        };

        // Report progress
        if let Ok(mut status) = job_store.get_job_status(&job_id) {
            status.progress = 50.0;
            let _ = job_store.save_job_status(&status);
        }

        let amplified_samples: Vec<Complex32> = samples.iter().map(|iq| iq * a).collect();

        // Convert back to bytes for DataObject
        let mut bytes = Vec::with_capacity(amplified_samples.len() * 8);
        for iq in amplified_samples {
            bytes.extend_from_slice(&iq.re.to_le_bytes());
            bytes.extend_from_slice(&iq.im.to_le_bytes());
        }

        let base64_data = base64::engine::general_purpose::STANDARD.encode(bytes);

        let mut output = iqengine_plugin::server::Output::new();
        output.data_output = Some(vec![iqengine_plugin::server::DataObject::new(
            iqengine_plugin::server::DataType::IqSlashCf32Le,
            request.metadata_file.file_name,
            base64_data,
        )]);

        Ok(output)
    }
}

pub const AMPLIFIER_FUNCTION: AmplifierFunction = AmplifierFunction {};
