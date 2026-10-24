# Third-party notices

Parla's own code is under the PolyForm Strict License 1.0.0 (see [LICENSE.md](LICENSE.md)).
The parts below come from other projects and stay under their own licenses.

## thinking-orbs

The dotted-sphere drawing in `src/overlay/orbEngine.ts` is adapted from
[thinking-orbs](https://github.com/Jakubantalik/thinking-orbs).

```
MIT License

Copyright (c) 2026 Jakub Antalik

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Dependencies

Parla is built with open-source libraries, including [Tauri](https://tauri.app),
[React](https://react.dev), [motion](https://motion.dev), [lucide](https://lucide.dev),
[swift-rs](https://github.com/Brendonovich/swift-rs) and
[speech-swift](https://github.com/soniqo/speech-swift). Each is used under its own
license, found in its repository. The speech models are NVIDIA Parakeet (CC-BY-4.0),
downloaded at first launch, and optionally OpenAI Whisper large-v3 turbo (MIT), downloaded
only if the user switches to it. Both are Core ML conversions published by
[aufklarer](https://huggingface.co/aufklarer) and used under their own licenses.

### Windows and Linux

The Windows and Linux builds also include [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)
(Apache-2.0) and [ONNX Runtime](https://github.com/microsoft/onnxruntime) (MIT) for speech,
[llama.cpp](https://github.com/ggml-org/llama.cpp) (MIT) for Enhance and Command Mode,
[cpal](https://github.com/RustAudio/cpal) (Apache-2.0) for the microphone and
[enigo](https://github.com/enigo-rs/enigo) (MIT) for typing, each under its own license. They
download, at first launch, the ONNX conversion of NVIDIA Parakeet TDT v3 (CC-BY-4.0) published
by [csukuangfj](https://huggingface.co/csukuangfj), and
[Qwen2.5 1.5B Instruct](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF) (Apache-2.0)
by the Qwen team.
