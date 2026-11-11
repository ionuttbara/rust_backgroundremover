# Background Remover AI 0.2.1 - Slint edition

This version keeps the original BiRefNet ONNX processing and adds a desktop Slint UI plus Windows Explorer Actions.

## GUI

Run the EXE without arguments. You can:
- drag & drop supported images or a folder into the window;
- choose multiple images with a file dialog;
- add a whole folder recursively;
- choose an output folder (or leave it as the same folder as each input);
- process in a worker thread while the GUI remains responsive;
- install the Explorer context menu from the GUI.

Output files are transparent PNG files named `original-name_nobg.png`.

## Multi-image processing rule

UI, Explorer Actions and CLI all use one common batch engine. Each processing request loads the ONNX model once, prepares and de-duplicates the complete image list, processes it in queue groups of at most two images (`2 + 2 + ... + 1`), then unloads the model after the complete queue. The pair grouping does not create a second model session, so memory/VRAM use stays predictable. A failed image is reported without cancelling the remaining queue.

## Explorer context menu

Install from the GUI or run:

```bat
rustbackremover_notintegrated.exe --add
```

Then right-click a supported image, a multi-selection of supported images, or a folder and use:

`Actions > Remove background`

Explorer may launch one process per item for legacy static verbs. The app coalesces near-simultaneous Explorer requests into one batch before loading the model.

The installer is per-user (`HKCU`) and also deletes the old `RemoveBackground` verb before registering the new cascade. Explorer is notified immediately after the registry change.

To remove the menu:

```bat
rustbackremover_notintegrated.exe --remove-context-menu
```

## AI Models Manager

The non-integrated build looks up:

`HKLM\SOFTWARE\Gallery Inc\AI-ModelsManager\models`

Value name:

`birefnetv2-img.onnx`

The integrated build embeds the model in the EXE. `build.bat` first looks for `birefnetv2-img.onnx` next to `Cargo.toml`; if it is not there, it attempts to copy it temporarily from AI Models Manager, builds the integrated EXE, then removes the temporary copy.

## Build outputs

- `rustbackremover_notintegrated.exe` - model loaded from AI Models Manager.
- `rustbackremover_integrated.exe` - model embedded in the EXE.

The app icon is embedded into the Windows executable by `build.rs`.

## Image formats

The original project listed JPG/JPEG, PNG, WebP, AVIF, HEIC and HEIF. The Rust `image` crate itself does not natively decode HEIC/HEIF, so those two formats still depend on adding a compatible decoder if your current build does not already provide one. JPG/JPEG, PNG, WebP and AVIF use the existing `image`-crate path.

## CLI batches

One file, multiple files, folders, or a mix can be supplied in the same command:

```bat
rustbackremover_notintegrated.exe image1.jpg image2.png "D:\Photos\Folder"
```

The same one-model / two-item queue-group rule is used. The Windows build does not open a separate Command Prompt for GUI or Explorer launches; when explicitly started from a terminal it attaches to that terminal for status output.
