use iqengine_plugin::server::{
    error::IQEngineError, Annotation, CustomParamType, FunctionParameters, FunctionParamsBuilder,
    FunctionPostRequest, FunctionPostResponse, SamplesB64Builder,
};
use num_complex::Complex32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplifierParams {
    #[serde(rename = "a")]
    a: f32,
}

pub struct AmplifierFunction {}

impl iqengine_plugin::server::IQFunction<AmplifierParams> for AmplifierFunction {
    fn parameters(self) -> FunctionParameters {
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

    fn apply(
        self,
        request: FunctionPostRequest<AmplifierParams>,
    ) -> Result<FunctionPostResponse, IQEngineError> {
        if let Some(samples_cloud) = request.samples_cloud {
            if !samples_cloud.is_empty() {
                return Err(IQEngineError::NotYetImplemented(
                    "Cloud samples not yet implemented".to_string(),
                ));
            }
        }
        if request.samples_b64.is_none() {
            return Err(IQEngineError::NotYetImplemented(
                "samples in Base64 are mandatory".to_string(),
            ));
        }

        let mut result = FunctionPostResponse::new();
        if let Some(samples_b64) = request.samples_b64 {
            let a = if let Some(prop) = request.custom_params {
                prop.a
            } else {
                return Err(IQEngineError::MandatoryParameter("a".to_string()));
            };
            let stream1 = samples_b64.get(0).unwrap();
            match stream1.data_type {
                iqengine_plugin::server::DataType::IqSlashCf32Le => {
                    let v = stream1.clone().samples_cf32()?;
                    let o = v.iter().map(|iq| iq * a);
                    let o: Vec<Complex32> = o.collect();
                    let output = SamplesB64Builder::same_as(stream1)
                        .with_samples_cf32(o)
                        .build()?;
                    result.data_output = Some(vec![output]);
                }
                _ => {
                    return Err(IQEngineError::UnsupportedDataType(stream1.data_type));
                }
            }
        }
        Ok(result)
    }
}

pub const AMPLIFIER_FUNCTION: AmplifierFunction = AmplifierFunction {};
