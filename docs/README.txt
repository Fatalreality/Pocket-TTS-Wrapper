================================================================================
                           POCKET TTS - USER GUIDE & INSTRUCTIONS
                       Lightweight CPU Text-to-Speech by Kyutai
================================================================================

Welcome to Pocket TTS! 
Pocket TTS is an ultra-fast, local CPU-based neural text-to-speech engine featuring
a modern Windows 11 Fluent Acrylic frosted glass interface, 27 natural built-in 
voices, and zero-shot voice cloning capabilities.

--------------------------------------------------------------------------------
TABLE OF CONTENTS
--------------------------------------------------------------------------------
1. QUICK START
2. HUGGING FACE ACCESS TOKEN (FOR VOICE CLONING)
3. BUILT-IN VOICES & GENDER FILTERING
4. HOW TO USE VOICE CLONING (.SAFETENSORS & AUDIO)
5. ADVANCED AUDIO CONTROLS & OPTIMAL SETTINGS
6. INTEGRATED DEVELOPER TERMINAL
7. KEYBOARD SHORTCUTS
8. TROUBLESHOOTING & FAQ

================================================================================
1. QUICK START
================================================================================
1. Launch Pocket TTS by double-clicking "pocket-tts-app.exe" or via your Start Menu.
2. In the "Generate" card on the left:
   - Type or paste the text you want spoken into the text box.
3. In the "Voice" card in the center:
   - Click any voice button (e.g., "cosette", "alba", "marius").
4. Click the large blue "Generate speech" button (or press Ctrl + Enter).
5. The audio will synthesize in real time and begin playing automatically!
6. Click "Save WAV" in the playback area to export the generated speech to disk.

================================================================================
2. HUGGING FACE ACCESS TOKEN (REQUIRED FOR VOICE CLONING)
================================================================================
IMPORTANT: Pocket TTS does NOT require a Hugging Face token for standard TTS using 
the 27 built-in voices. They work 100% offline immediately!

A Hugging Face token is ONLY required if you want to use arbitrary VOICE CLONING 
(recording your own voice or importing an external audio file to clone). Kyutai 
hosts the voice-cloning neural encoder weights as a gated model on Hugging Face.

STEP-BY-STEP SETUP (Takes less than 2 minutes):

Step 1: Accept the Model License
   - Visit the official repository in your browser:
     https://huggingface.co/kyutai/pocket-tts
   - If prompted, log into your free Hugging Face account.
   - Click "Agree and access repository" to accept the research terms.

Step 2: Generate a User Access Token
   - Go to your Hugging Face Access Tokens page:
     https://huggingface.co/settings/tokens
   - Click "Create new token" (or "New token").
   - Give it any name (e.g. "PocketTTS").
   - Type / Permissions: Select "Read" access.
   - Click "Generate token" and copy the token starting with "hf_...".

Step 3: Save Your Token in Pocket TTS
   - In Pocket TTS, look at the bottom of the "Voice Clone" card on the right.
   - You will see the "Hugging Face Access" box.
   - Paste your "hf_..." token into the input box.
   - Click the "Save" button.
   - The badge will immediately update to green: "✓ Token Saved" or "✓ Cloning Ready".
   - That's it! Your token is securely persisted in ~/.cache/huggingface/token.
     You will never have to enter it again.

================================================================================
3. BUILT-IN VOICES & GENDER FILTERING
================================================================================
Pocket TTS comes with 27 predefined voice personalities. 

GENDER FILTER PILLS:
At the top of the Voice card, use the filter pills to quickly view:
  - "All"     : Displays all 27 voices.
  - "♀ Female": Filters down to natural feminine voices.
  - "♂ Male"  : Filters down to natural masculine voices.

RECOMMENDED VOICES:
  - Natural Feminine:
    * cosette   (Clear, expressive storytelling)
    * alba      (Gentle, articulate)
    * anna      (Warm conversational)
    * vera      (Crisp, clear broadcast)
    * fantine   (Deep, emotional narrative)
    * eve       (American standard)
    * jane      (British refined)
  - Natural Masculine:
    * marius    (Casual, modern tone)
    * javert    (Deep, authoritative)
    * george    (British classic)
    * paul      (Friendly narrative)
    * michael   (Articulate conversational)

================================================================================
4. HOW TO USE VOICE CLONING (.SAFETENSORS & AUDIO)
================================================================================
Pocket TTS gives you two ways to clone voices:

OPTION A: Direct Microphone Recording
1. In the "Voice Clone" card, click "Record voice" (red dot starts pulsing).
2. Speak clearly into your microphone for 5 to 10 seconds.
3. Click "Stop recording".
4. A save dialog will appear to save your WAV recording.
5. The recording is instantly set as your active voice.
6. Type your text and click "Generate speech"!

OPTION B: Import an Existing Audio File or Voice Model
1. Click "Import audio file…" to select any .wav, .mp3, .flac, or .ogg file.
   OR:
   Click "Import voice clone (.safetensors)…" to load a previously exported voice.
2. The file name will appear in the chip badge.
3. Click "Generate speech" to speak using that voice.

WHAT IS A .SAFETENSORS VOICE CLONE?
- When you click "Export voice clone (.safetensors)…", Pocket TTS computes the 
  mathematical voice embeddings and saves them to a compact .safetensors file.
- Loading a .safetensors file is INSTANTANEOUS (takes milliseconds instead of 
  seconds) because no audio re-encoding is needed!
- You can share your .safetensors voice files with friends or reuse them anytime.

================================================================================
5. ADVANCED AUDIO CONTROLS & OPTIMAL SETTINGS
================================================================================
In the "Advanced" card on the bottom-right:

- Temperature (Default: 0.3)
  * Controls vocal expressiveness and pitch variation.
  * NOTE: Kyutai recommends 0.3 for optimal, natural speech clarity. Higher values
    (e.g., 0.7+) can cause pitch instability and raspy artifacts.

- EOS Threshold (Default: -4.0)
  * Controls end-of-sentence detection sensitivity.

- Sampler Decode Steps (Default: 1)
  * Number of diffusion sampling steps per audio frame.

- Max Tokens (Default: 50)
  * Maximum chunk length processed per synthesis iteration.

- Noise Clamp (Default: unset)
  * Audio noise suppression filter.

================================================================================
6. INTEGRATED DEVELOPER TERMINAL
================================================================================
No external command prompt windows are opened when running Pocket TTS.
Instead, a sleek collapsible Terminal Drawer is embedded at the very bottom 
of the application window.

- Click the terminal bar at the bottom to expand/collapse live logs.
- Features real-time log streaming (INFO, WARN, ERROR, and TTS audio stats).
- Includes "Copy" and "Clear" buttons for easy debugging.

================================================================================
7. KEYBOARD SHORTCUTS
================================================================================
- Ctrl + Enter (or Cmd + Enter) : Generate speech from current text
- Space                         : Play / Pause audio playback

================================================================================
8. TROUBLESHOOTING & FAQ
================================================================================
Q: I get "We could not download the weights for the model with voice cloning".
A: Follow Section 2 above to accept the terms at https://huggingface.co/kyutai/pocket-tts
   and paste your Hugging Face token into the in-app Hugging Face Access box.

Q: The voices sound raspy or too deep.
A: Ensure your "Temperature" slider in the Advanced card is set to 0.3 (the calibrated 
   optimal setting) and make sure you have selected a voice matching your desired gender.

Q: Where does Pocket TTS store downloaded model files?
A: Standard Hugging Face cache: ~/.cache/huggingface/hub/

================================================================================
Enjoy creating speech with Pocket TTS!
================================================================================
