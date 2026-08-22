# 第三方模型声明

本目录下的模型文件由第三方提供，**不随代码分发**，由 `scripts/download-kws-model.sh`
按 `manifest.json` 清单按需下载。清单记录了来源 URL 与 sha256 校验和。

## sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20

- **用途**: 中英混合关键词唤醒词检测（KWS）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `68447f4fbc67e70eee3a93961f36e81e98f47aef73ce7e7ca00885c6cd3616a6`

## sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01

- **用途**: 纯英文关键词唤醒词检测（KWS）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目，模型由 pkufool 训练，GigaSpeech XL 1 万小时）
- **许可证**: Apache-2.0（依据 sherpa-onnx 项目整体许可；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `f170013b4716e41b62b9bfd809687c207cef798ef9bc6534d524e17af9b6561a`
- **测试夹具**: `src/kws/testdata/bpe.model` 取自该模型包内的同名单文件（239KB），
  用于钉住子词切分行为，随代码分发，许可同上

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

## sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17

- **用途**: 离线多语言语音识别（ASR，zh/en/ja/ko/yue，含情绪/事件标签；int8 轻量版）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目；模型源自阿里 ModelScope iic/SenseVoiceSmall）
- **许可证**: FunASR Model License v1.1（阿里）；如需商用请以官方模型发布页的许可说明为准
- **sha256**: `7d1efa2138a65b0b488df37f8b89e3d91a60676e416f515b952358d83dfd347e`

## sherpa-onnx-whisper-tiny

- **用途**: 离线多语言语音识别（ASR，OpenAI Whisper tiny）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目；模型源自 OpenAI Whisper）
- **许可证**: MIT（OpenAI Whisper；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `c46116994e539aa165266d96b325252728429c12535eb9d8b6a2b10f129e66b1`

## sherpa-onnx-whisper-base

- **用途**: 离线多语言语音识别（ASR，OpenAI Whisper base）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2
- **发布方**: k2-fsa（sherpa-onnx 项目；模型源自 OpenAI Whisper）
- **许可证**: MIT（OpenAI Whisper；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `911b2083efd7c0dca2ac3b358b75222660dc09fb716d64fbfc417ba6c99ff3de`

## silero_vad.onnx（离线听写 VAD）

- **用途**: 离线免提听写的语音活动检测（VAD，说/静音分段）
- **来源**: https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
- **发布方**: k2-fsa（sherpa-onnx 项目；模型源自 Silero Team）
- **许可证**: MIT（Silero VAD；如需商用请以官方模型发布页的许可说明为准）
- **sha256**: `9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6`
