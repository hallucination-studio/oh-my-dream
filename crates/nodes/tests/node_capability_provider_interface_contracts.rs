use nodes::{
    ImageToVideoProviderFakeImpl, TextToImageProviderFakeImpl, TextToSpeechProviderFakeImpl,
};

#[path = "c5_support/provider_contracts.rs"]
mod provider_contracts;
use provider_contracts::{
    assert_image_to_video_provider_contract, assert_text_to_image_provider_contract,
    assert_text_to_speech_provider_contract,
};

#[tokio::test]
async fn text_to_image_provider_fake_impl_satisfies_provider_contract() {
    assert_text_to_image_provider_contract(&TextToImageProviderFakeImpl::try_new().unwrap()).await;
}

#[tokio::test]
async fn image_to_video_provider_fake_impl_satisfies_provider_contract() {
    assert_image_to_video_provider_contract(&ImageToVideoProviderFakeImpl::try_new().unwrap())
        .await;
}

#[tokio::test]
async fn text_to_speech_provider_fake_impl_satisfies_provider_contract() {
    assert_text_to_speech_provider_contract(&TextToSpeechProviderFakeImpl::try_new().unwrap())
        .await;
}
