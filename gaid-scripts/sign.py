#!/usr/bin/env python3
"""
Script to sign a serialized PSET using the user key for a specific GAID in the given environment on Liquid testnet.

This script uses the GDK (Green Development Kit) Python bindings to:
1. Take environment, GAID, and serialized PSET as arguments.
2. Login to the wallet using the environment's mnemonic.
3. Verify the GAID exists in the wallet's subaccounts.
4. Sign the PSET using the wallet's keys (adds user signature for multisig).
5. Output JSON with the signed PSET.

Usage:
    python sign.py <environment> <gaid> <serialized_pset>

    environment: 'dev', 'test', or 'amp'
    gaid: The Green Account ID to verify against.
    serialized_pset: Base64-encoded serialized PSET to sign.
"""

import json
import sys
import time
import green_gdk as gdk

def main():
    if len(sys.argv) != 4:
        print("Usage: python sign.py <environment> <gaid> <serialized_pset>")
        sys.exit(1)

    environment = sys.argv[1].lower()
    gaid = sys.argv[2]
    serialized_pset = sys.argv[3]

    valid_environments = ['dev', 'test', 'amp']
    if environment not in valid_environments:
        print(f"Error: Invalid environment '{environment}'")
        print(f"Valid environments are: {', '.join(valid_environments)}")
        sys.exit(1)

    print(f"Using environment: {environment}, GAID: {gaid}")

    # Initialize GDK
    gdk.init({
        'datadir': '.',     # Use current directory for any state files
        'log_level': 'warn'
    })

    # Environment-specific mnemonics - DO NOT use these for real funds!
    mnemonics = {
        'dev': "vote ball voice juice visit license music off paddle hold suffer beach someone hover wheat boring protect broccoli issue opinion uniform media update arrange",
        'test': "hood novel finish couch rack pistol march army gun bring universe unknown hurry famous vote enact inspire whisper violin blur chief into engage cigar",
        'amp': "perfect grit monkey disorder cliff exhibit meat giant dream secret liberty walnut later caught snow wasp bulb bind feature couple vault flower credit bundle"
    }

    mnemonic = mnemonics[environment]

    # Validate the mnemonic
    if not gdk.validate_mnemonic(mnemonic):
        raise Exception("Invalid mnemonic")

    # Create session for Liquid testnet
    session = gdk.Session({'name': 'testnet-liquid'})

    # Prepare credentials
    credentials = {'mnemonic': mnemonic}

    # Helper function to login with retry
    def login_with_retry(session, credentials, max_retries=3):
        for attempt in range(max_retries):
            try:
                session.login_user({}, credentials).resolve()
                print("Successfully logged in")
                return session
            except RuntimeError as e:
                if 'reconnect required' in str(e) and attempt < max_retries - 1:
                    print(f"Connection error, retrying... (attempt {attempt + 2}/{max_retries})")
                    time.sleep(2)
                    # Recreate session
                    session = gdk.Session({'name': 'testnet-liquid'})
                else:
                    raise
        return session

    session = login_with_retry(session, credentials)

    # Get existing subaccounts and verify GAID exists
    subaccounts_result = session.get_subaccounts().resolve()
    found_subaccount = False
    for subaccount in subaccounts_result['subaccounts']:
        if subaccount['receiving_id'] == gaid and subaccount['type'] == '2of2_no_recovery':
            found_subaccount = True
            print(f"Found matching subaccount for GAID: {gaid} (pointer: {subaccount['pointer']})")
            break

    if not found_subaccount:
        print(f"Error: GAID {gaid} not found in environment {environment}")
        sys.exit(1)

    # Sign the PSET with retry logic
    max_retries = 3
    for attempt in range(max_retries):
        try:
            signed_result = session.psbt_sign(serialized_pset).resolve()
            signed_pset = signed_result['pset']
            break
        except RuntimeError as e:
            if 'reconnect required' in str(e) and attempt < max_retries - 1:
                print(f"Connection error during signing, retrying... (attempt {attempt + 2}/{max_retries})")
                time.sleep(2)
                # Re-login
                session = login_with_retry(gdk.Session({'name': 'testnet-liquid'}), credentials)
            else:
                raise Exception(f"Error signing PSET: {e}")

    # Output JSON
    output = {"signed_pset": signed_pset}
    print(json.dumps(output, indent=2))

if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"\nError: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
