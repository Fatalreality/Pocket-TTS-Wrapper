================================================================================
                           POCKET TTS - USER GUIDE & INSTRUCTIONS
                       Lightweight CPU Text-to-Speech by Kyutai
================================================================================

Welcome to Pocket TTS!
Pocket TTS is an ultra-fast, local CPU-based neural text-to-speech engine with a
frosted-glass Windows interface, 27 built-in voices and zero-shot voice cloning.

--------------------------------------------------------------------------------
TABLE OF CONTENTS
--------------------------------------------------------------------------------
1. QUICK START
2. HUGGING FACE ACCESS TOKEN (FOR VOICE CLONING)
3. BUILT-IN VOICES, PREVIEWS & GENDER FILTERING
4. HOW TO USE VOICE CLONING (.SAFETENSORS & AUDIO)
5. ADVANCED AUDIO CONTROLS & OPTIMAL SETTINGS
6. SAVING AUDIO, LONG-FORM RENDERING AND RECENT CLIPS
7. SPEAKING TEXT FROM OTHER APPS (TRAY & SHORTCUT)
8. UPDATES, CONSOLE & SAVED SETTINGS
9. KEYBOARD SHORTCUTS
10. TROUBLESHOOTING & FAQ

================================================================================
1. QUICK START
================================================================================
1. Launch Pocket TTS from the Start menu, or run the portable
   "Pocket-TTS-<version>-portable.exe" from wherever you saved it.
2. Type or paste the text you want spoken into "Text to speak".
3. Click a voice in the Voice card (e.g. "cosette", "alba", "marius").
4. Click "Generate speech" (or press Ctrl + Enter). Audio is synthesized in
   real time and starts playing automatically.
5. Click "Save audio" to export it as WAV, MP3 or Ogg Vorbis.

The model loads on first use and the first launch downloads the weights from
Hugging Face. Later launches reuse them from the local cache in
~/.cache/huggingface/hub/.

While generation runs the button turns into "Stop": pressing it ends the job at
the next chunk and keeps the audio finished so far.

================================================================================
2. HUGGING FACE ACCESS TOKEN (FOR VOICE CLONING)
================================================================================
IMPORTANT: Pocket TTS does NOT require a Hugging Face token to use the 27
built-in voices. They work 100% offline immediately!

A token is ONLY needed for arbitrary VOICE CLONING: the voice-cloning encoder
weights are a gated model on Hugging Face.

STEP 1: Accept the model license
   - Visit https://huggingface.co/kyutai/pocket-tts
   - Log in, then click "Agree and access repository".

STEP 2: Create a Read token
   - Go to https://huggingface.co/settings/tokens
   - Click "Create new token", choose "Read" access, then copy the "hf_...".

STEP 3: Save it in Pocket TTS
   - At the bottom of the "Voice Clone" card, paste the token into
     "Hugging Face Access" and click "Save".
   - The badge changes to "Token Saved" or "Cloning Ready".
   - The token is stored in ~/.cache/huggingface/token and reused.

================================================================================
3. BUILT-IN VOICES, PREVIEWS & GENDER FILTERING
================================================================================
GENDER FILTER PILLS at the top of the Voice card:
  - "All"        : all 27 voices.
  - "Female"     : natural feminine voices.
  - "Male"       : natural masculine voices.

HEARING A VOICE: with "Hear a voice when it is picked" ticked (Voice card), the
short sample "Hello, this is how I sound." is generated and played whenever you
pick a built-in voice. Samples are cached for the session, so clicking the same
voice again is instant. Untick the box if you would rather not spend the compute.

RECOMMENDED VOICES:
  Natural feminine: cosette, alba, anna, vera, fantine, eponine, eve, jane
  Natural masculine: marius, javert, jean, charles, paul, george, michael

Voices follow the selected language in the Model card; other languages have
their own default speakers.

================================================================================
4. HOW TO USE VOICE CLONING (.SAFETENSORS & AUDIO)
================================================================================
OPTION A: Microphone recording
1. Click "Record voice" (the dot pulses), speak clearly for 5-10 seconds.
2. Click "Stop recording" and choose where to save the WAV.
3. The recording becomes the active voice.

OPTION B: Existing audio or a saved voice model
1. "Import audio file…" accepts .wav, .mp3, .flac, .ogg and .m4a.
   "Import voice clone (.safetensors)…" loads a previously exported voice.
2. The file name appears in the chip; generate speech as usual.

WHAT IS A .SAFETENSORS VOICE CLONE?
- "Export voice clone (.safetensors)…" computes the voice embeddings and saves
  them to a compact file.
- Loading one is instant (milliseconds) because no audio has to be re-encoded.
- The files can be shared and reused on any Pocket TTS install.

================================================================================
5. ADVANCED AUDIO CONTROLS & OPTIMAL SETTINGS
================================================================================
In the Advanced card, bottom right:

- Temperature (Default: 0.3)
  Vocal expressiveness and pitch variation. Kyutai recommends 0.3; values of
  0.7+ can cause pitch instability and raspy artifacts.

- EOS threshold (Default: -4.0)
  Sensitivity of end-of-speech detection.

- Sampler decode steps (Default: 1)
  Diffusion sampling steps per audio frame; more steps cost speed.

- Max tokens (Default: 50)
  Maximum chunk length per synthesis pass.

- Noise clamp (Default: unset)
  Audio noise suppression filter; move the slider to enable it.

- Continue from previous chunk
  Teacher-forces each chunk from the previous one for smoother prosody across
  long texts. A little slower.

All of these, together with the language, selected voice and gender filter, are
saved automatically and restored on the next launch.

================================================================================
6. SAVING AUDIO, LONG-FORM RENDERING AND RECENT CLIPS
================================================================================
SAVE AUDIO: the save dialog's file type picks the format.
  - .wav  : uncompressed, largest
  - .mp3  : 192 kbps, small and universal
  - .ogg  : Ogg Vorbis, small and open

LONG-FORM RENDERING: "Render a text file…" turns a whole .txt or .md into one
audio file. It streams to disk (a book costs no more memory than a sentence) and
shows progress per rendered second.

STOPPING AND RESUMING A RENDER:
  - Press "Stop" at any time; the file keeps everything rendered so far.
  - "Resume unfinished render" then continues from the next unfinished sentence,
    even after restarting the app, and appends to the same output file.
  - Progress is stored next to the output as "<name>.resume.json" together with
    "<name>.resume.pcm"; both are deleted when the render completes.

RECENT CLIPS: every generation in this session is listed under "Recent" with its
voice and length. Click one to load it back into the player and export it again
without synthesizing it a second time.

================================================================================
7. SPEAKING TEXT FROM OTHER APPS (TRAY & SHORTCUT)
================================================================================
With Pocket TTS running you can read text from anywhere:

1. Copy the text you want spoken (Ctrl + C) in any application.
2. Press Ctrl + Alt + S, or right-click the Pocket TTS tray icon and choose
   "Speak clipboard (Ctrl+Alt+S)".
3. Pocket TTS comes to the front, puts the text in the box and generates it.

The tray icon also has "Show Pocket TTS" and "Quit". If another program already
owns Ctrl+Alt+S, the app logs a warning in the console and the tray entry still
works.

================================================================================
8. UPDATES, CONSOLE & SAVED SETTINGS
================================================================================
UPDATES: the download icon in the title bar checks the latest release. When a
newer version exists the app downloads the signed installer, verifies it and
restarts into it. The installer is small; model weights are reused.

CONSOLE & LOGS: the collapsible drawer at the bottom shows live engine logs.
  - "Save" writes the transcript to a file, with a header naming the build,
    platform and CPU thread count - attach it to a bug report.
  - "Copy" copies the transcript, "Clear" empties it.

SAVED SETTINGS: theme, surface mode, language, voice, advanced sliders and the
preview toggle are stored per user and restored on launch.

================================================================================
9. KEYBOARD SHORTCUTS
================================================================================
- Ctrl + Enter          : Generate speech
- Space                 : Play / pause (when not typing in a field)
- Ctrl + Alt + S        : Speak the clipboard text (global, works in any app)

================================================================================
10. TROUBLESHOOTING & FAQ
================================================================================
Q: I get "We could not download the weights for the model with voice cloning".
A: Accept the terms at https://huggingface.co/kyutai/pocket-tts and paste a
   Read token into the in-app Hugging Face Access box (Section 2).

Q: The voices sound raspy or too deep.
A: Set Temperature back to 0.3 and pick a voice matching the gender you want.

Q: Where are downloaded model files stored?
A: In the standard Hugging Face cache: ~/.cache/huggingface/hub/

Q: Pocket TTS does not start at all.
A: Windows needs the Microsoft Edge WebView2 runtime. The app offers its
   download page when that is the cause; installing it and starting the app
   again fixes it.

Q: A render was interrupted. Do I have to start over?
A: No. "Resume unfinished render" continues from the last finished sentence and
   appends to the same file. Cancel the resume, if you prefer to start fresh.

Q: Why did generation stop early?
A: Either "Stop" was pressed or the model hit its maximum length without an end
   of sentence; check the console for a warning.

================================================================================
Enjoy creating speech with Pocket TTS!
================================================================================
