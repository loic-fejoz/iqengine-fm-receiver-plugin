use iqengine_plugin::server::{
    error::IQEngineError, Annotation, CustomParamType, FunctionParameters, FunctionParamsBuilder,
    FunctionPostRequest, FunctionPostResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmReceiverParams {
    #[serde(rename = "target_freq")]
    target_freq: f32,
}

pub struct FmReceiverFunction {}

impl iqengine_plugin::server::IQFunction<FmReceiverParams> for FmReceiverFunction {
    fn parameters(self) -> FunctionParameters {
        FunctionParamsBuilder::new()
            .max_inputs(1)
            .max_outputs(1)
            .custom_param(
                "target_freq",
                "Center of FM carrier",
                CustomParamType::Number,
                Some("0.0"),
            )
            .build()
    }

    fn apply(
        self,
        request: FunctionPostRequest<FmReceiverParams>,
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
        let mut first_annot = Annotation::new(100, 10);
        first_annot.core_colon_label = Some("random detection".into());
        first_annot.core_colon_comment = Some("from rust plugin".into());
        let mut annotations = Vec::new();
        annotations.push(first_annot);
        result.annotations = Some(annotations);
        Ok(result)
    }
}

pub const FM_RECEIVER_FUNCTION: FmReceiverFunction = FmReceiverFunction {};
