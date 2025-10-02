#!/usr/bin/env python3
import green_gdk as gdk

# Initialize GDK
gdk.init({
    'datadir': '.',
    'log_level': 'warn'
})

# Generate a new mnemonic
mnemonic = gdk.generate_mnemonic()
print(f"Generated mnemonic: {mnemonic}")

# Validate it
if gdk.validate_mnemonic(mnemonic):
    print("Mnemonic is valid!")
else:
    print("Mnemonic is invalid!")
