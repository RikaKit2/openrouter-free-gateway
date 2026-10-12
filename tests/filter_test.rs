use openrouter_free_gateway::models::{
    IneligibilityReason, ModelEligibility, ModelTag, PricingTier, TokenPrice, ToolCapability,
};

#[test]
fn test_token_price_parsing() {
    assert_eq!(TokenPrice::parse("0"), TokenPrice::Free);
    assert_eq!(TokenPrice::parse("0.0"), TokenPrice::Free);
    assert_eq!(TokenPrice::parse("0.000000"), TokenPrice::Free);
    assert_eq!(TokenPrice::parse(""), TokenPrice::Free);
    assert_eq!(TokenPrice::parse("0.000001"), TokenPrice::Paid);
    assert_eq!(TokenPrice::parse("1.5"), TokenPrice::Paid);
}

#[test]
fn test_pricing_tier() {
    assert_eq!(
        PricingTier::from_rates("0", "0"),
        PricingTier::CompletelyFree
    );
    assert_eq!(
        PricingTier::from_rates("0.001", "0"),
        PricingTier::InputPaidOutputFree
    );
    assert_eq!(
        PricingTier::from_rates("0", "0.002"),
        PricingTier::InputFreeOutputPaid
    );
    assert_eq!(PricingTier::from_rates("0.001", "0.002"), PricingTier::Paid);
    assert!(PricingTier::CompletelyFree.is_completely_free());
    assert!(!PricingTier::InputPaidOutputFree.is_completely_free());
}

#[test]
fn test_model_tag() {
    assert_eq!(
        ModelTag::from_id("nvidia/nemotron-3.5-lightning:free"),
        ModelTag::FreeTagged
    );
    assert_eq!(ModelTag::from_id("openrouter/free"), ModelTag::FreeTagged);
    assert_eq!(
        ModelTag::from_id("dots-studio/dots-3-note-preview:free"),
        ModelTag::FreeTagged
    );
    assert_eq!(ModelTag::from_id("openai/gpt-4o"), ModelTag::Standard);
    assert_eq!(
        ModelTag::from_id("anthropic/claude-3-5-sonnet"),
        ModelTag::Standard
    );
}

#[test]
fn test_tool_capability() {
    assert_eq!(
        ToolCapability::from_parameters(&["tools".to_string(), "temperature".to_string()]),
        ToolCapability::Supported
    );
    assert_eq!(
        ToolCapability::from_parameters(&["temperature".to_string()]),
        ToolCapability::Unsupported
    );
    assert_eq!(
        ToolCapability::from_parameters(&[]),
        ToolCapability::Unsupported
    );
}

#[test]
fn test_model_eligibility() {
    // 100% Free with tag and tools -> Eligible
    assert_eq!(
        ModelEligibility::evaluate(
            PricingTier::CompletelyFree,
            ModelTag::FreeTagged,
            ToolCapability::Supported
        ),
        ModelEligibility::Eligible
    );

    // Paid input -> Ineligible
    assert_eq!(
        ModelEligibility::evaluate(
            PricingTier::InputPaidOutputFree,
            ModelTag::FreeTagged,
            ToolCapability::Supported
        ),
        ModelEligibility::Ineligible(IneligibilityReason::NonZeroPricing(
            PricingTier::InputPaidOutputFree
        ))
    );

    // Paid output -> Ineligible
    assert_eq!(
        ModelEligibility::evaluate(
            PricingTier::InputFreeOutputPaid,
            ModelTag::FreeTagged,
            ToolCapability::Supported
        ),
        ModelEligibility::Ineligible(IneligibilityReason::NonZeroPricing(
            PricingTier::InputFreeOutputPaid
        ))
    );

    // Missing free tag -> Ineligible
    assert_eq!(
        ModelEligibility::evaluate(
            PricingTier::CompletelyFree,
            ModelTag::Standard,
            ToolCapability::Supported
        ),
        ModelEligibility::Ineligible(IneligibilityReason::MissingFreeTag)
    );

    // Missing tools -> Ineligible
    assert_eq!(
        ModelEligibility::evaluate(
            PricingTier::CompletelyFree,
            ModelTag::FreeTagged,
            ToolCapability::Unsupported
        ),
        ModelEligibility::Ineligible(IneligibilityReason::MissingTools)
    );
}
#[test]
fn test_filter_free_models_catalog() {
    use openrouter_free_gateway::models::{OpenRouterModel, OpenRouterPricing, filter_free_models};

    let models = vec![
        OpenRouterModel {
            id: "nvidia/nemotron-3.5-lightning:free".to_string(),
            name: "Nemotron 3.5 Lightning (Free)".to_string(),
            pricing: OpenRouterPricing {
                prompt: "0".to_string(),
                completion: "0".to_string(),
            },
            context_length: Some(1_000_000),
            top_provider: None,
            supported_parameters: vec!["tools".to_string()],
            architecture: None,
        },
        OpenRouterModel {
            id: "openai/gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
            pricing: OpenRouterPricing {
                prompt: "0.005".to_string(),
                completion: "0.015".to_string(),
            },
            context_length: Some(128_000),
            top_provider: None,
            supported_parameters: vec!["tools".to_string()],
            architecture: None,
        },
        OpenRouterModel {
            id: "google/gemma-2-9b:free".to_string(),
            name: "Gemma 2 9B Free without tools".to_string(),
            pricing: OpenRouterPricing {
                prompt: "0".to_string(),
                completion: "0".to_string(),
            },
            context_length: Some(8192),
            top_provider: None,
            supported_parameters: vec!["temperature".to_string()],
            architecture: None,
        },
    ];

    let filtered = filter_free_models(&models);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "nemotron-3.5-lightning:free");
    assert_eq!(
        filtered[0].upstream_id.as_deref(),
        Some("nvidia/nemotron-3.5-lightning:free")
    );
}
