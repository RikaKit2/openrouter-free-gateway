use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenPrice {
    Free,
    Paid,
}

impl TokenPrice {
    #[must_use]
    pub fn parse(val: &str) -> Self {
        match val.trim() {
            "0" | "0.0" | "0.00" | "0.000" | "0.0000" | "0.00000" | "0.000000" | "" => Self::Free,
            _ => match val.trim().parse::<f64>() {
                Ok(num) => match num.abs() < f64::EPSILON {
                    true => Self::Free,
                    false => Self::Paid,
                },
                Err(_) => Self::Paid,
            },
        }
    }

    #[must_use]
    pub const fn is_free(self) -> bool {
        matches!(self, Self::Free)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PricingTier {
    CompletelyFree,
    InputPaidOutputFree,
    InputFreeOutputPaid,
    Paid,
}

impl PricingTier {
    #[must_use]
    pub fn from_rates(prompt_rate: &str, completion_rate: &str) -> Self {
        match (
            TokenPrice::parse(prompt_rate),
            TokenPrice::parse(completion_rate),
        ) {
            (TokenPrice::Free, TokenPrice::Free) => Self::CompletelyFree,
            (TokenPrice::Paid, TokenPrice::Free) => Self::InputPaidOutputFree,
            (TokenPrice::Free, TokenPrice::Paid) => Self::InputFreeOutputPaid,
            (TokenPrice::Paid, TokenPrice::Paid) => Self::Paid,
        }
    }

    #[must_use]
    pub const fn is_completely_free(self) -> bool {
        matches!(self, Self::CompletelyFree)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTag {
    FreeTagged,
    Standard,
}

impl ModelTag {
    #[must_use]
    pub fn from_id(id: &str) -> Self {
        match () {
            _ if id.ends_with(":free") || id == "openrouter/free" => Self::FreeTagged,
            _ => Self::Standard,
        }
    }

    #[must_use]
    pub const fn is_free_tagged(self) -> bool {
        matches!(self, Self::FreeTagged)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCapability {
    Supported,
    Unsupported,
}

impl ToolCapability {
    #[must_use]
    pub fn from_parameters(parameters: &[String]) -> Self {
        match parameters.iter().any(|param| param == "tools") {
            true => Self::Supported,
            false => Self::Unsupported,
        }
    }

    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IneligibilityReason {
    NonZeroPricing(PricingTier),
    MissingFreeTag,
    MissingTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelEligibility {
    Eligible,
    Ineligible(IneligibilityReason),
}

impl ModelEligibility {
    #[must_use]
    pub fn evaluate(pricing: PricingTier, tag: ModelTag, tools: ToolCapability) -> Self {
        match (pricing, tag, tools) {
            (PricingTier::CompletelyFree, ModelTag::FreeTagged, ToolCapability::Supported) => {
                Self::Eligible
            }
            (PricingTier::InputPaidOutputFree, _, _) => Self::Ineligible(
                IneligibilityReason::NonZeroPricing(PricingTier::InputPaidOutputFree),
            ),
            (PricingTier::InputFreeOutputPaid, _, _) => Self::Ineligible(
                IneligibilityReason::NonZeroPricing(PricingTier::InputFreeOutputPaid),
            ),
            (PricingTier::Paid, _, _) => {
                Self::Ineligible(IneligibilityReason::NonZeroPricing(PricingTier::Paid))
            }
            (PricingTier::CompletelyFree, ModelTag::Standard, _) => {
                Self::Ineligible(IneligibilityReason::MissingFreeTag)
            }
            (PricingTier::CompletelyFree, ModelTag::FreeTagged, ToolCapability::Unsupported) => {
                Self::Ineligible(IneligibilityReason::MissingTools)
            }
        }
    }
    #[must_use]
    pub const fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterPricing {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub completion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterTopProvider {
    pub max_completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterArchitecture {
    pub modality: Option<String>,
    #[serde(default)]
    pub input_modalities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub pricing: OpenRouterPricing,
    pub context_length: Option<u64>,
    pub top_provider: Option<OpenRouterTopProvider>,
    #[serde(default)]
    pub supported_parameters: Vec<String>,
    pub architecture: Option<OpenRouterArchitecture>,
}

impl OpenRouterModel {
    #[must_use]
    pub fn check_eligibility(&self) -> ModelEligibility {
        let pricing = PricingTier::from_rates(&self.pricing.prompt, &self.pricing.completion);
        let tag = ModelTag::from_id(&self.id);
        let tools = ToolCapability::from_parameters(&self.supported_parameters);
        ModelEligibility::evaluate(pricing, tag, tools)
    }

    #[must_use]
    pub fn short_id(&self) -> &str {
        match self.id.split_once('/') {
            Some((_, short)) => short,
            None => &self.id,
        }
    }

    #[must_use]
    pub fn to_openai_model(&self) -> OpenAIModel {
        OpenAIModel {
            id: self.short_id().to_string(),
            object: "model".to_string(),
            created: 1_700_000_000,
            owned_by: "openrouter".to_string(),
            context_length: self.context_length,
            max_completion_tokens: self
                .top_provider
                .as_ref()
                .and_then(|provider| provider.max_completion_tokens),
            upstream_id: Some(self.id.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAIModel {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAIModelList {
    pub object: String,
    pub data: Vec<OpenAIModel>,
}

#[must_use]
pub fn filter_free_models(models: &[OpenRouterModel]) -> Vec<OpenAIModel> {
    models
        .iter()
        .filter_map(|model| match model.check_eligibility() {
            ModelEligibility::Eligible => Some(model.to_openai_model()),
            ModelEligibility::Ineligible(_) => None,
        })
        .collect()
}
