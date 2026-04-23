#!/usr/bin/env python3
"""
ONNX INT8 Dynamic Quantization Script for OpenLife Layer-1 Intent Router.

Usage:
    python scripts/quantize_int8.py --input ./models/intent.onnx --output ./models/intent_int8.onnx

Requires:
    pip install onnx onnxruntime
"""

import argparse
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description="Quantize ONNX model to INT8")
    parser.add_argument("--input", required=True, help="Path to input ONNX model")
    parser.add_argument("--output", required=True, help="Path to output INT8 ONNX model")
    args = parser.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)

    if not input_path.exists():
        raise FileNotFoundError(f"Input model not found: {input_path}")

    try:
        from onnxruntime.quantization import quantize_dynamic, QuantType
    except ImportError as e:
        raise ImportError(
            "onnxruntime is required for quantization. "
            "Install it with: pip install onnxruntime"
        ) from e

    output_path.parent.mkdir(parents=True, exist_ok=True)

    quantize_dynamic(
        model_input=str(input_path),
        model_output=str(output_path),
        weight_type=QuantType.QInt8,
        optimize_model=True,
    )

    original_size = input_path.stat().st_size / 1024 / 1024
    quantized_size = output_path.stat().st_size / 1024 / 1024
    print(f"[quantize_int8] Original model size: {original_size:.2f} MB - quantize_int8.py:47")
    print(f"[quantize_int8] Quantized model size: {quantized_size:.2f} MB - quantize_int8.py:48")
    print(f"[quantize_int8] Saved to: {output_path} - quantize_int8.py:49")


if __name__ == "__main__":
    main()
