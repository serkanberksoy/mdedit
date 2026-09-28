#!/usr/bin/env python3
"""Runs mdedit on a pty inside Konsole, relaying its output to Konsole
verbatim and typing scripted keys, so the real terminal's screen can be read
back over D-Bus (`qdbus ... getAllDisplayedTextList`). Used by
tests/konsole.rs; Konsole blocks sending keys over D-Bus, hence the pty.

usage: konsole_drive.py APP FILE KEYS DONE_FILE
  KEYS: d = Down, u = Up, e = End, h = Home, E = Ctrl+E (one char per key)
  DONE_FILE: written with "ROWSxCOLS" once all keys are processed
"""
import fcntl, os, pty, select, struct, sys, termios, time, tty

KEYS = {"d": b"\x1b[B", "u": b"\x1b[A", "e": b"\x1b[F", "h": b"\x1b[H", "E": b"\x05"}


def main():
    app, file, keys, done = sys.argv[1:5]
    rows, cols, _, _ = struct.unpack("hhhh", fcntl.ioctl(1, termios.TIOCGWINSZ, b"\0" * 8))
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(app, [app, file])
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("hhhh", rows, cols, 0, 0))
    tty.setraw(0)

    def pump(seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.02)
            if ready:
                try:
                    os.write(1, os.read(fd, 65536))
                except OSError:
                    return

    pump(0.8)
    for k in keys:
        os.write(fd, KEYS[k])
        pump(0.06)
    pump(0.5)
    with open(done, "w") as f:
        f.write(f"{rows}x{cols}\n")
    pump(600)  # keep the screen up until the test closes the window


if __name__ == "__main__":
    main()
