# 第三方模型声明

本目录下的模型文件由第三方提供，**不随代码分发**，由 `scripts/download-kws-model.sh`
按 `manifest.json` 清单按需下载。清单记录了来源 URL 与 sha256 校验和。

## sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20

- **用途**: 中英混合关键词唤醒词检测（KWS）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `68447f4fbc67e70eee3a93961f36e81e98f47aef73ce7e7ca00885c6cd3616a6`

## sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20

- **用途**: 中英双语流式语音识别（ASR）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目，模型由社区贡献）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `27ffbd9ee24ad186d99acc2f6354d7992b27bcab490812510665fa8f9389c5f8`

## sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12

- **用途**: 中英双语标点恢复（ASR 结果自动加标点）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/punctuation-models/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目，源自阿里 DAMO Academy 的 CT-Transformer 标点模型）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `50f73f8cccffc2303999fda28b785ffcffbd7ea442c47385c30b9d045ee6afc3`

## sherpa-onnx-zipvoice-distill-int8-zh-en-emilia

- **用途**: 中英双语文本转语音（TTS，ZipVoice 零样本声音克隆）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-zipvoice-distill-int8-zh-en-emilia.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `77219c8b40f4ee8d73a7f902305ff6c1128ef9b54461c41b4ca6ed890b6c2803`

## vocos_24khz.onnx（TTS 声码器）

- **用途**: ZipVoice TTS 的声码器（vocoder，把 mel 谱转成波形）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/vocoder-models/vocos_24khz.onnx
- **发布方**: k2-fsa（sherpa-onnx 项目）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `bcb3b970e384161c4d634f0bb9e999ff1c471b34c9bc0b1049a5014065ed3cc0`
