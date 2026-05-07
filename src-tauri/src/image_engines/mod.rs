pub(crate) mod gemini;
pub(crate) mod openai;

use crate::model_registry;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ImageProvider {
    OpenAi,
    Gemini,
}

pub(crate) fn provider_for_model(model: &str) -> ImageProvider {
    if model_registry::is_gemini_model(model) {
        ImageProvider::Gemini
    } else {
        ImageProvider::OpenAi
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn provider_selection_accepts_gemini_aliases_and_provider_ids() {
        assert_eq!(
            provider_for_model(ENGINE_NANO_BANANA),
            ImageProvider::Gemini
        );
        assert_eq!(
            provider_for_model(ENGINE_NANO_BANANA_2),
            ImageProvider::Gemini
        );
        assert_eq!(
            provider_for_model(ENGINE_NANO_BANANA_PRO),
            ImageProvider::Gemini
        );
        assert_eq!(
            provider_for_model(GEMINI_MODEL_NANO_BANANA),
            ImageProvider::Gemini
        );
        assert_eq!(
            provider_for_model(GEMINI_MODEL_NANO_BANANA_2),
            ImageProvider::Gemini
        );
        assert_eq!(
            provider_for_model(GEMINI_MODEL_NANO_BANANA_PRO),
            ImageProvider::Gemini
        );
        assert_eq!(
            provider_for_model(ENGINE_GPT_IMAGE_2),
            ImageProvider::OpenAi
        );
    }

    #[test]
    fn provider_selection_matches_model_registry_classification() {
        for model in [
            ENGINE_GPT_IMAGE_2,
            ENGINE_NANO_BANANA,
            ENGINE_NANO_BANANA_2,
            ENGINE_NANO_BANANA_PRO,
            GEMINI_MODEL_NANO_BANANA,
            GEMINI_MODEL_NANO_BANANA_2,
            GEMINI_MODEL_NANO_BANANA_PRO,
            "unknown-model",
        ] {
            assert_eq!(
                provider_for_model(model) == ImageProvider::Gemini,
                model_registry::is_gemini_model(model),
                "provider routing should share model registry classification for {model}"
            );
        }
    }
}
