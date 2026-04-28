use anyhow::{Context, Result};
use ndarray::{s, Array2};
use std::path::Path;
use std::time::Instant;
use tokenizers::Tokenizer;
use tract_onnx::prelude::*;

use crate::router::Intent;

type TypedPlan = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

pub struct ReflexEngine {
    model: TypedPlan,
    tokenizer: Tokenizer,
    labels: Vec<String>,
}

impl ReflexEngine {
    pub fn new(
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
        labels: Vec<String>,
    ) -> Result<Self> {
        let model = tract_onnx::onnx()
            .model_for_path(model_path.as_ref())
            .context("failed to load ONNX model")?
            .into_optimized()
            .context("failed to optimize ONNX model")?
            .into_runnable()
            .context("failed to make ONNX model runnable")?;
        let tokenizer = Tokenizer::from_file(tokenizer_path.as_ref())
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;
        Ok(Self {
            model,
            tokenizer,
            labels,
        })
    }

    /// Try to classify using ONNX. Returns (Intent, latency_us).
    pub fn classify(&self, text: &str) -> Result<(Intent, u64)> {
        let start = Instant::now();
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("tokenize error: {e}"))?;
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let len = ids.len();

        let input_ids: Tensor = Array2::from_shape_vec((1, len), ids)
            .context("input_ids tensor creation failed")?
            .into();
        let attention_mask: Tensor = Array2::from_shape_vec((1, len), mask)
            .context("attention_mask tensor creation failed")?
            .into();

        let outputs = self
            .model
            .run(tvec!(input_ids.into(), attention_mask.into()))
            .context("ONNX inference failed")?;

        let logits = outputs[0]
            .to_array_view::<f32>()
            .context("failed to view logits")?;
        let view = logits.view();
        let last_token_logits: Vec<f32> = if view.ndim() == 3 {
            let seq_len = view.shape()[1];
            view.slice(s!(0, seq_len - 1, ..)).to_vec()
        } else if view.ndim() == 2 {
            view.slice(s!(0, ..)).to_vec()
        } else {
            vec![]
        };

        let mut best_idx = 0usize;
        let mut best_score = f32::MIN;
        for (i, &score) in last_token_logits.iter().enumerate() {
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }

        let label = self
            .labels
            .get(best_idx)
            .map(|s| s.as_str())
            .unwrap_or("complex");
        let intent = Self::label_to_intent(label);
        let latency = start.elapsed().as_micros() as u64;
        Ok((intent, latency))
    }

    fn label_to_intent(label: &str) -> Intent {
        match label {
            "greeting" => Intent::Greeting,
            "goodbye" => Intent::Goodbye,
            "help" => Intent::Help,
            "life_model_query" => Intent::LifeModelQuery,
            "small_talk" => Intent::SmallTalk,
            "tool_request" => Intent::ToolRequest,
            "sensitive" => Intent::Sensitive,
            _ => Intent::Complex,
        }
    }

    /// Load quantized model if `intent_int8.onnx` exists in the same directory,
    /// otherwise fall back to `intent.onnx`.
    pub fn new_with_optional_quantization(
        model_dir: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
        labels: Vec<String>,
    ) -> Result<Self> {
        let dir = model_dir.as_ref();
        let quantized = dir.join("intent_int8.onnx");
        let standard = dir.join("intent.onnx");
        if quantized.exists() {
            match Self::new(&quantized, &tokenizer_path, labels.clone()) {
                Ok(engine) => {
                    eprintln!("[ReflexEngine] Loaded INT8 quantized model. - reflex_engine.rs:123");
                    return Ok(engine);
                }
                Err(e) => {
                    eprintln!(
                        "[ReflexEngine] Failed to load INT8 model ({}), falling back to standard.",
                        e
                    );
                }
            }
        }
        Self::new(&standard, &tokenizer_path, labels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn load_fallback_when_int8_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = ReflexEngine::new_with_optional_quantization(
            dir.path(),
            dir.path().join("tokenizer.json"),
            vec!["complex".into()],
        );
        assert!(result.is_err());
    }

    /// Benchmark latency of the ONNX engine.
    /// Ignored by default because it requires a real model.
    /// Run with: cargo test -- --ignored
    #[test]
    #[ignore = "requires real ONNX model files"]
    fn classify_latency_p99_under_50ms() {
        let model_dir = std::env::var("MODEL_DIR").unwrap_or_else(|_| "./models".into());
        let engine = ReflexEngine::new_with_optional_quantization(
            &model_dir,
            std::path::Path::new(&model_dir).join("tokenizer.json"),
            vec![
                "greeting".into(),
                "goodbye".into(),
                "help".into(),
                "life_model_query".into(),
                "small_talk".into(),
                "tool_request".into(),
                "sensitive".into(),
                "complex".into(),
            ],
        )
        .expect("Failed to load model");

        let samples = vec![
            "你好",
            "再见",
            "help",
            "我的人生模型是什么",
            "今天天气不错",
            "帮我查一下文件",
            "我的手机号是13800138000",
            "这是一个比较复杂的长句子，需要深入推理。",
        ];

        let iterations = 100;
        let mut latencies: Vec<u64> = Vec::with_capacity(iterations * samples.len());

        for _ in 0..iterations {
            for text in &samples {
                let start = Instant::now();
                let _ = engine.classify(text).unwrap();
                latencies.push(start.elapsed().as_micros() as u64);
            }
        }

        latencies.sort_unstable();
        let p99_idx = (latencies.len() as f64 * 0.99) as usize;
        let p99 = latencies[p99_idx.min(latencies.len() - 1)];
        eprintln!(
            "P99 latency: {}us ({}ms) - reflex_engine.rs:202",
            p99,
            p99 / 1000
        );
        assert!(p99 < 50_000, "P99 latency {}us exceeds 50ms threshold", p99);
    }
}
