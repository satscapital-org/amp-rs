#!/usr/bin/env python3
"""
Script to retrieve the next unused address for a given GAID in the specified environment on Liquid testnet.

This script uses the GDK (Green Development Kit) Python bindings to:
1. Accept environment and GAID as command-line arguments.
2. Login to the wallet using the environment's mnemonic.
3. Verify the GAID exists in the wallet's subaccounts.
4. Retrieve the next unused address for the subaccount associated with the GAID.
5. Output JSON with the address.

Usage:
    python address.py [options] <environment> <gaid>

    Options:
        -v, --verbose    Show detailed output including progress messages

    environment: 'dev', 'test', or 'amp'
    gaid: The Green Account ID to get the next address for.

    Examples:
        python address.py dev GA2p1mdft7v3KuMGbzY5wT1Ma8AGc     # JSON output only
        python address.py -v test GA4AjdGmT4yTUrrLScLe13BMPV9sXD  # Verbose output
"""

import json
import sys
import time
import os
import green_gdk as gdk

def main():
    # Parse command line arguments
    verbose = False
    original_stderr = None
    args = sys.argv[1:]

    # Check for verbose flag
    if '-v' in args or '--verbose' in args:
        verbose = True
        # Remove verbose flag from args
        args = [arg for arg in args if arg not in ['-v', '--verbose']]

    # Validate remaining arguments
    if len(args) != 2:
        print("Usage: python address.py [options] <environment> <gaid>")
        print("Options:")
        print("  -v, --verbose    Show detailed output")
        sys.exit(1)

    environment = args[0].lower()
    gaid = args[1]

    valid_environments = ['dev', 'test', 'amp']
    if environment not in valid_environments:
        print(f"Error: Invalid environment '{environment}'")
        print(f"Valid environments are: {', '.join(valid_environments)}")
        sys.exit(1)

    if verbose:
        print(f"Using environment: {environment}, GAID: {gaid}")

    # Initialize GDK
    # In non-verbose mode, suppress all GDK output including warnings
    if not verbose:
        # Redirect stderr to suppress certificate warnings
        original_stderr = sys.stderr
        sys.stderr = open(os.devnull, 'w')

    try:
        gdk.init({
            'datadir': '.',     # Use current directory for any state files
            'log_level': 'none' if not verbose else 'warn'
        })
    except:
        # Even if init has issues with log_level, continue
        gdk.init({
            'datadir': '.',
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
    def login_with_retry(session, credentials, verbose, max_retries=3):
        for attempt in range(max_retries):
            try:
                session.login_user({}, credentials).resolve()
                if verbose:
                    print("Successfully logged in")
                return session
            except RuntimeError as e:
                if 'reconnect required' in str(e) and attempt < max_retries - 1:
                    if verbose:
                        print(f"Connection error, retrying... (attempt {attempt + 2}/{max_retries})")
                    time.sleep(2)
                    # Recreate session
                    session = gdk.Session({'name': 'testnet-liquid'})
                else:
                    raise
        return session

    session = login_with_retry(session, credentials, verbose)

    # Find the subaccount matching the GAID
    subaccounts_result = session.get_subaccounts().resolve()
    found_subaccount = False
    subaccount_pointer = None
    for subaccount in subaccounts_result['subaccounts']:
        if subaccount['receiving_id'] == gaid and subaccount['type'] == '2of2_no_recovery':
            found_subaccount = True
            subaccount_pointer = subaccount['pointer']
            if verbose:
                print(f"Found matching subaccount for GAID: {gaid} (pointer: {subaccount_pointer})")
            break

    if not found_subaccount:
        error_msg = f"GAID {gaid} not found in environment {environment}"
        if verbose:
            print(f"Error: {error_msg}")
        else:
            # Output error as JSON
            print(json.dumps({"error": error_msg}))
        # Restore stderr before exiting
        if not verbose and original_stderr is not None:
            sys.stderr.close()
            sys.stderr = original_stderr
        sys.exit(1)

    # Retrieve the next unused address
    max_retries = 3
    for attempt in range(max_retries):
        try:
            addr_details = session.get_receive_address({'subaccount': subaccount_pointer}).resolve()
            address = addr_details['address']
            break
        except RuntimeError as e:
            if 'reconnect required' in str(e) and attempt < max_retries - 1:
                if verbose:
                    print(f"Connection error during address retrieval, retrying... (attempt {attempt + 2}/{max_retries})")
                time.sleep(2)
                # Re-login
                session = login_with_retry(gdk.Session({'name': 'testnet-liquid'}), credentials, verbose)
            else:
                raise Exception(f"Error retrieving address: {e}")

    # Output JSON
    output = {"address": address}
    print(json.dumps(output, indent=2))

    # Restore stderr if we redirected it
    if not verbose and original_stderr is not None:
        sys.stderr.close()
        sys.stderr = original_stderr

if __name__ == "__main__":
    # Check if verbose mode
    verbose = '-v' in sys.argv or '--verbose' in sys.argv

    try:
        main()
    except Exception as e:
        if verbose:
            print(f"\nError: {e}")
            import traceback
            traceback.print_exc()
        else:
            # In non-verbose mode, output a valid JSON error
            print(json.dumps({"error": str(e)}))
        sys.exit(1)
