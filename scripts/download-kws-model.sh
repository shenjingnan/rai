#!/usr/bin/env bash
# 下载 sherpa-onnx KWS 唤醒词模型到 ./models/（幂等，已存在则跳过）。
set -euo pipefail

MODEL_NAME="sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODEL_DIR="$REPO_ROOT/models"
URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/${MODEL_NAME}.tar.bz2"

if [ -d "$MODEL_DIR/$MODEL_NAME" ]; then
  echo "模型已存在: $MODEL_DIR/$MODEL_NAME"
  exit 0
fi

mkdir -p "$MODEL_DIR"
echo "下载 $URL ..."
curl -fSL -O "$URL"
tar xjf "${MODEL_NAME}.tar.bz2" -C "$MODEL_DIR"
rm -f "${MODEL_NAME}.tar.bz2"
echo "完成: $MODEL_DIR/$MODEL_NAME"
