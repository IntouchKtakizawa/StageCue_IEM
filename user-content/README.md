# Your StageCue content

This folder is deliberately separate from application source code.

- Put backing-track WAV files in `audio/`.
- Put your own recorded voice cues in `cues/`.
- Put editable song projects in `projects/`.

In StageCue, click **Backing → Import WAV backing** or **Voice cues → Import my recorded cue**. The project stores the portable relative path shown above, so copy the selected recording into the matching folder before performing.

Voice cue files should preferably be mono WAV at 48 kHz. Clear filenames such as `chorus-two-bars.wav` make projects easier to repair if files are moved.

The JSON format is documented in `docs/schema.md`. Start by copying `fixtures/standard-rock.stagecue.json` into `projects/`, then edit the copy—never edit fixtures when making a real show file.
