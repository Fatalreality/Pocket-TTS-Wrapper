# Vendored `ptts`

Copy of the `ptts` crate from <https://github.com/gabrieldemarmiesse/xn-ptts> at rev
`7efa26973f81bba42cf2ace8da2c1eb9b605d54d` (version `0.4.0-rc.3`), MIT OR Apache-2.0.

It is vendored so the app can carry one small addition that the pinned revision does not expose:
teacher-forced chunk continuation, which the Python `pocket-tts` package offers as
`teacher_force_previous_chunk`. Only `src/` and the two license files are copied; the workspace
crate manifests are replaced by the standalone `Cargo.toml` next to this file.

Changes against upstream (the only ones):

- `src/flow_lm.rs`: `FlowLM::bos_embedding()` exposes the BOS frame as a `[1, 1, ldim]` latent.
- `src/tts_model.rs`: `TTSModel::prompt_text_and_latents()` prefills the flow LM with the text
  tokens and a latent sequence (BOS, then every latent but the last) in a single pass, mirroring
  the Python `_run_flow_lm_and_increment_step(..., backbone_input_latents=...)` call.

When bumping the upstream revision, re-copy `src/` and reapply those two additions.
