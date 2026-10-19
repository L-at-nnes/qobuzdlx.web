#!/bin/sh
# Receives one event as a line of JSON on the standard input. This one needs nothing but sh: it
# writes the whole line, which a real extension would read with jq or a script in its language.
# The container image has no shell: this example is for a QobuzDLX run outside the image, or in an
# image that has one. See docs/extensions.md.
cat >> "$QOBUZDLX_EXTENSION_DIR/saved.log"
