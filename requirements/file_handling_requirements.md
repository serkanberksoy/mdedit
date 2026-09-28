# File Handling Requirements

Opening, saving, "Save As" and exiting in mdedit. All of these are in
Milestone 1 (file operations).

Status: F-01 ✅ · F-02 ✅ · F-03 ✅ · F-04 ✅ (0.9.0) · F-05 ✅ (0.38.0) · F-06 ✅ (0.40.0) · F-07 ✅ (0.50.2)

## POC behavior before these requirements (for reference)

- Ctrl+S saves to the file given on the command line. If mdedit was started
  without a file, it only shows "No file name" in the status bar.
- Ctrl+Q quits. If there are unsaved changes, the first press warns in the
  status bar and a second press quits without saving.
- There is no Save As and no way to choose a file name or folder from inside
  the editor.
- A file can only be opened from the command line (`mdedit FILE`), not from
  inside the editor.

---

## Requirements

- **F-01 Ctrl+S: Save** ✅ P1
  - If the document already has a file (it was opened from disk, or saved
    earlier in this session), Ctrl+S writes it straight away with no prompt,
    and the status bar confirms (`Saved <path>`).
  - If the document has no file yet (a new, untitled document), Ctrl+S
    opens the Save As popup (F-02) instead.
  - On a write error (permissions, missing folder, disk full), the document
    stays marked as unsaved and the error shows in the status bar.

- **F-02 Ctrl+Shift+S: Save As** ✅ P1 (also **Ctrl+Alt+S**, which works in every terminal)
  Opens a popup over the editor where the user can:
  - type or edit the **file name**, pre-filled with the current name, or
    `untitled.md` for a new document. `.md` is added if no extension is
    given.
  - choose the **folder**: it starts in the current file's folder (or the
    working directory for a new document), and the user can move up with
    `..` and into subfolders from a list. Typing a path directly
    (e.g. `~/notes/2026-06-30.md`) also works.
  - **Enter** saves, **Esc** cancels and goes back to editing with nothing
    changed.
  - If the target file already exists, ask for confirmation before
    overwriting it: `Overwrite <name>? (y/n)`.
  - After a successful Save As, the document belongs to the new path: the
    title and status bar show it, and later Ctrl+S saves there.

- **F-03 Ctrl+X: Exit** ✅ P1
  - If there are no unsaved changes, mdedit exits immediately.
  - If there are unsaved changes, a popup asks:
    `Save changes to <name>?  [Y]es  [N]o  [C]ancel`
    - **Yes**: saves the way Ctrl+S does (F-01): straight to the document's
      file, or through the Save As popup (F-02) if it's untitled (see open
      question 1). If that save succeeds, mdedit exits. If the user cancels
      Save As or the write fails, mdedit returns to the editor and doesn't
      exit.
    - **No**: exit without saving.
    - **Cancel** / Esc: return to the editor.
  - Ctrl+X replaces Ctrl+Q as the exit key.

- **F-04 Ctrl+O: Open file** ✅ P1
  - **Unsaved changes are handled first.** If the current document has
    unsaved changes, a popup asks *before* the file picker opens:
    `Save changes to <name>?  [Y]es  [N]o  [C]ancel`
    - **Yes**: save the document the way Ctrl+S does (F-01): straight to its
      file, or through the Save As popup (F-02) if it's untitled. The file
      picker opens only if the save succeeds. If Save As is cancelled or the
      write fails, stay in the current document.
    - **No**: go on to the file picker without saving. The changes are
      discarded only when another file is actually opened; if the picker is
      cancelled, the document is still there with its changes (see Esc
      below).
    - **Cancel** / Esc: return to the editor; nothing changes.
    - With no unsaved changes, Ctrl+O goes straight to the file picker.
  - **File picker popup:**
    - Starts in the current file's folder (or the working directory for an
      untitled document).
    - Lists subfolders first, then Markdown files (`.md`, `.markdown`),
      sorted by name. `..` goes up a folder. A toggle (e.g. Ctrl+H) shows
      all files, including hidden ones.
    - Up/Down selects; Enter on a folder enters it; Enter on a file opens
      it. Typing filters the list by name. Typing a path directly (e.g.
      `~/notes/2026-06-30.md`) also works.
    - **Esc** cancels and returns to the current document unchanged,
      including any unsaved changes, even after *No* at the save prompt. The
      prompt says "No: discard the changes when another file is opened".
    - The picker and the F-02 Save As folder browser share one component.
  - **After a file is chosen:**
    - The file replaces the current document: cursor at the top, scroll
      reset, status bar `Opened <path>`, document not marked as unsaved.
      Later Ctrl+S saves to this file.
    - If the file can't be read (permissions, not UTF-8 text, it's a
      folder), show the error in the status bar and stay in the picker. The
      current document isn't touched.
    - Choosing the file that's already open reloads it from disk.

- **F-05 Ctrl+Enter: Follow link** ✅ P1 (feature row K-12)
  - With the cursor on a link, **Ctrl+Enter** opens the linked Markdown
    file. **Alt+Enter** does the same, for terminals that can't tell
    Ctrl+Enter from Enter.
  - Links: `[[Note]]`, `[[Note|alias]]`, `[[Folder/Note]]`,
    `[[Note#Heading]]` and Markdown links `[text](Note.md)`,
    `[text](Note%20name.md)`. The cursor can be anywhere on the link.
  - **Resolution (no vault):** relative to the current file's folder (the
    working directory for an untitled document). `.md` is added when the
    target has no extension. mdedit never searches other folders; a wrapper
    project can do vault-wide resolution later.
  - A `#Heading` part moves the cursor to that heading in the opened file.
    `[[#Heading]]` jumps within the current file.
  - **Unsaved changes:** a popup explains that the link is being followed:
    `Following link to <target>. Save changes to <name>? [Y]es [N]o [C]ancel`.
    Yes saves (Save As if untitled) and then follows the link; No discards
    the changes and follows it; Cancel stays.
  - A link to a missing file, or a web URL, isn't followed; the status bar
    says why.

- **F-06 Open at a section** ✅ P1
  - `mdedit note.md#My Title` opens the note with the cursor on that
    heading, and the heading at the top of the screen. Following a
    `[[Note#Heading]]` link (F-05) lands the same way.
  - Headings match ignoring case, or ignoring everything but letters and
    digits: `#my-title` and `#mytitle` both find "My Title".
  - A file whose name really contains `#` is opened as-is. An unknown
    heading opens the file at the top, and the status bar says so.

- **F-07 Safe save** ✅ P1
  - Saving never leaves a half-written file: the text goes to a temporary
    file in the same folder, which then replaces the note in one step. A
    failed save (full disk, no permission) leaves the old file untouched.
  - The file keeps its line endings (LF or CRLF) and whether it ends with a
    newline; a new file uses LF and ends with one.
  - Saving through a symbolic link writes the file it points to (the link
    stays), and an existing file keeps its permissions.

---

## Open questions / risks

1. **Yes on an already-saved document:** the request says Yes opens the Save
   As popup. For a document that already has a file, would you rather Yes
   save straight to that file, as Ctrl+S does, and exit? The recommended
   default is: Save As only for untitled documents, a direct save otherwise.
   **Implemented as the recommended default** (0.8.0), the same rule as
   F-04's Yes. Easy to change if Save As is wanted every time.
2. **Ctrl+Shift+S may be indistinguishable from Ctrl+S:** in the standard
   terminal key encoding, Ctrl+Shift+S and Ctrl+S send the same byte, so
   the editor can't tell them apart. Telling them apart needs the kitty
   keyboard protocol, which crossterm supports as "keyboard enhancement
   flags". Supporting terminals include kitty, WezTerm, foot, Ghostty and
   recent Alacritty; Konsole's support must be tested. Requirement: turn the
   protocol on when it's available, and always offer a fallback key that
   works everywhere (proposal: **F12** or **Ctrl+Alt+S**), listed in the
   status bar help.
   **Decided:** the fallback is **Ctrl+Alt+S** (F12 is taken by Yakuake on
   KDE). mdedit turns on the protocol's `DISAMBIGUATE_ESCAPE_CODES` flag when
   the terminal supports it.
   **Konsole (verified, 26.08):** Konsole supports the kitty protocol
   (it answers `CSI ?0u`), **but it binds Ctrl+Shift+S to its own "Save
   Output As…"**, which opens a KDE file dialog and never passes the key to
   mdedit. To use Ctrl+Shift+S in Konsole, remove or change that shortcut
   (Konsole menu → Settings → Configure Keyboard Shortcuts… → "Save Output
   As…"). Ctrl+Alt+S works either way.
3. **Ctrl+X is usually "cut"** in editors. Using it for exit (as nano does)
   means clipboard cut needs a different key later (e.g. Ctrl+Shift+X or
   Alt+X).
4. **Ctrl+O in terminals:** some terminals reserve Ctrl+O (e.g. the
   `discard` character in some tty settings). Raw mode normally passes it
   through; verify it in Konsole with the scripted harness.
5. **Ctrl+S in terminals:** some terminals pause output on Ctrl+S (XON/XOFF
   flow control). The raw mode mdedit runs in turns this off, and Ctrl+S
   saved correctly in the scripted POC tests. Keep it covered by a test.
