use crate::{ChatProvider, ChatRequest, ChatResponse, ProviderError};
use std::sync::Mutex;

pub struct MockProvider {
    responses: Vec<ChatResponse>,
    calls: Mutex<Vec<ChatRequest>>,
}

impl MockProvider {
    pub fn scripted(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<ChatRequest> {
        self.calls
            .lock()
            .expect("mock call log is not poisoned")
            .clone()
    }
}

impl ChatProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let mut calls = self.calls.lock().expect("mock call log is not poisoned");
        calls.push(request.clone());
        let index = calls.len() - 1;
        drop(calls);
        self.responses.get(index).cloned().ok_or_else(|| {
            ProviderError::InvalidResponse("mock provider script exhausted".to_string())
        })
    }
}
