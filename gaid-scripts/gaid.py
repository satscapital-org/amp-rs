#!/usr/bin/env python3
"""
Script to create 20 2of2_no_recovery subaccounts on Liquid testnet and display their GAIDs.

This script uses the GDK (Green Development Kit) Python bindings to:
1. Create a session connected to Liquid testnet
2. Register/login with a test mnemonic
3. Create 20 2of2_no_recovery subaccounts (used for Liquid AMP)
4. Display all GAIDs (Green Account IDs)

Usage:
    python gaid.py [options] [environment]

    Options:
        -v, --verbose    Show detailed output including all account information
    
    environment: 'dev', 'test', or 'amp' (defaults to 'dev')
    
    Examples:
        python gaid.py                    # Default (dev environment, JSON only)
        python gaid.py test               # Test environment, JSON only
        python gaid.py -v dev             # Dev environment with verbose output
        python gaid.py --verbose amp      # AMP environment with verbose output
"""

import json
import sys
import time
import os
import green_gdk as gdk

def main():
    # Parse command line arguments
    valid_environments = ['dev', 'test', 'amp']
    environment = 'dev'  # default
    verbose = False
    original_stderr = None
    
    # Process command line arguments
    args = sys.argv[1:]
    
    # Check for verbose flag
    if '-v' in args or '--verbose' in args:
        verbose = True
        # Remove verbose flag from args
        args = [arg for arg in args if arg not in ['-v', '--verbose']]
    
    # Process environment argument
    if len(args) > 0:
        environment = args[0].lower()
        if environment not in valid_environments:
            print(f"Error: Invalid environment '{environment}'")
            print(f"Valid environments are: {', '.join(valid_environments)}")
            sys.exit(1)

    if verbose:
        print(f"Using environment: {environment}")

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
    # These are deterministic test mnemonics for each environment
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
    # For mainnet Liquid, use 'liquid' instead of 'testnet-liquid'
    session = gdk.Session({'name': 'testnet-liquid'})

    # Prepare credentials
    credentials = {'mnemonic': mnemonic}

    # Helper function to login with retry
    def login_with_retry(session, credentials, verbose, max_retries=3):
        for attempt in range(max_retries):
            try:
                # Try to login first (in case wallet already exists)
                try:
                    if verbose:
                        print("Attempting to login to existing wallet...")
                    session.login_user({}, credentials).resolve()
                    if verbose:
                        print("Successfully logged in to existing wallet")
                    return session
                except RuntimeError as e:
                    if 'id_login_failed' in str(e):
                        # If login fails, register a new wallet
                        if verbose:
                            print("Wallet doesn't exist, registering new wallet...")
                        session.register_user({}, credentials).resolve()
                        session.login_user({}, credentials).resolve()
                        if verbose:
                            print("Successfully registered and logged in")
                        return session
                    else:
                        raise
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

    # Get existing subaccounts
    subaccounts_result = session.get_subaccounts().resolve()
    existing_subaccounts = {}

    # Build a map of existing subaccounts by name
    for subaccount in subaccounts_result['subaccounts']:
        if subaccount['type'] == '2of2_no_recovery':
            existing_subaccounts[subaccount['name']] = subaccount

    # Create or retrieve 20 subaccounts
    gaids = []
    if verbose:
        print("\nCreating/retrieving 20 subaccounts...")

    for i in range(1, 21):
        account_name = f"GAID {environment.upper()} Account {i:02d}"

        if account_name in existing_subaccounts:
            # Use existing subaccount
            subaccount = existing_subaccounts[account_name]
            pointer = subaccount['pointer']
            gaid = subaccount['receiving_id']
            if verbose:
                print(f"  Found existing subaccount {i:2d}: {account_name}")
        else:
            # Create new 2of2_no_recovery subaccount with retry logic
            if verbose:
                print(f"  Creating new subaccount {i:2d}: {account_name}")
            create_details = {
                'name': account_name,
                'type': '2of2_no_recovery'
            }

            # Try creating the subaccount with retry on connection errors
            max_retries = 3
            for attempt in range(max_retries):
                try:
                    # Create the subaccount
                    result = session.create_subaccount(create_details).resolve()
                    pointer = result['pointer']

                    # Fetch the subaccount details to get the GAID
                    subaccount_details = session.get_subaccount(pointer).resolve()
                    gaid = subaccount_details['receiving_id']
                    break
                except RuntimeError as e:
                    if 'reconnect required' in str(e) and attempt < max_retries - 1:
                        if verbose:
                            print(f"    Connection error, retrying... (attempt {attempt + 2}/{max_retries})")
                        time.sleep(2)
                        # Re-login
                        try:
                            session.login_user({}, credentials).resolve()
                        except:
                            session = gdk.Session({'name': 'testnet-liquid'})
                            session.login_user({}, credentials).resolve()
                    else:
                        raise

        # Get address details to extract public keys and private keys
        user_privkey = 'N/A'
        user_pubkey = 'N/A'
        service_pubkey = 'N/A'

        try:
            addr_details = session.get_receive_address({'subaccount': pointer}).resolve()
            script = addr_details.get('script', '')

            # Get private key for the subaccount
            try:
                # For 2of2_no_recovery accounts, we need to get the wallet private key
                # Try getting wallet details that might include key information

                # Method 1: Try to get private key through address details with show_priv_key flag
                try:
                    # Some GDK versions support getting private keys directly
                    addr_with_priv = session.get_receive_address({
                        'subaccount': pointer,
                        'show_priv_key': True  # This flag might expose private keys
                    }).resolve()

                    if 'priv_key' in addr_with_priv:
                        user_privkey = addr_with_priv['priv_key']
                    elif 'private_key' in addr_with_priv:
                        user_privkey = addr_with_priv['private_key']
                except:
                    pass

                # Method 2: Try to get from previous addresses that might have private keys
                if user_privkey == 'N/A':
                    try:
                        # Get previous addresses which might include private key info
                        prev_addrs = session.get_previous_addresses({
                            'subaccount': pointer,
                            'last_pointer': 10  # Get first 10 addresses
                        }).resolve()

                        if 'list' in prev_addrs:
                            for addr_info in prev_addrs['list']:
                                if 'private_key' in addr_info:
                                    user_privkey = addr_info['private_key']
                                    break
                                elif 'priv_key' in addr_info:
                                    user_privkey = addr_info['priv_key']
                                    break
                    except:
                        pass

                # Method 3: Use get_subaccount_root_path to derive keys manually
                if user_privkey == 'N/A':
                    try:
                        # Get the root path for the subaccount
                        root_path = session.get_subaccount_root_path(pointer).resolve()
                        # This would require BIP32 key derivation which GDK might not expose directly
                    except:
                        pass

                # Note: GDK intentionally limits private key access for security
                # In production, private keys should be handled very carefully
            except Exception as e:
                # Private key extraction failed, continue with public keys only
                if verbose:
                    print(f"    Note: Could not extract private key for account {i}")

            # Parse public keys from script
            # For 2of2_no_recovery, script format is: 52 21 <pubkey1> 21 <pubkey2> 52 ae
            if script and len(script) > 140:  # Minimum length for 2of2 script
                try:
                    # First pubkey starts after '5221' (OP_2 + 33 bytes indicator)
                    if script[:4] == '5221':
                        user_pubkey = script[4:4+66]  # 33 bytes = 66 hex chars
                        # Second pubkey starts after first pubkey + '21'
                        if script[70:72] == '21':
                            service_pubkey = script[72:72+66]
                except Exception as e:
                    if verbose:
                        print(f"    Warning: Could not parse script for account {i}: {e}")
        except Exception as e:
            pass

        gaids.append({
            'index': i,
            'name': account_name,
            'pointer': pointer,
            'gaid': gaid,
            'user_privkey': user_privkey,
            'user_pubkey': user_pubkey,
            'service_pubkey': service_pubkey
        })

    # Display detailed output only in verbose mode
    if verbose:
        # Display all GAIDs with public keys
        print("\n" + "=" * 100)
        print(f"20 Subaccounts with their GAIDs and Public Keys (Environment: {environment.upper()}):")
        print("=" * 100)

        for account in gaids:
            print(f"{account['index']:2d}. {account['name']:<25} GAID: {account['gaid']}")
            print(f"    User PrivKey: {account['user_privkey']}")
            print(f"    User PubKey: {account['user_pubkey']}")
            print(f"    Service PubKey: {account['service_pubkey']}")
            print()

        print("=" * 100)
        print(f"Total subaccounts: {len(gaids)}")
        print("=" * 100)
        
        print("\n" + "=" * 100)
        print("JSON Output:")
        print("=" * 100)
    
    # Create JSON output with just the GAIDs array
    gaid_list = [account['gaid'] for account in gaids]
    json_output = {
        'gaids': gaid_list
    }
    
    # Print JSON (raw in non-verbose mode, pretty in verbose mode)
    print(json.dumps(json_output, indent=2))
    
    if verbose:
        print("=" * 100)
    
    # Restore stderr if we redirected it
    if not verbose and original_stderr is not None:
        sys.stderr.close()
        sys.stderr = original_stderr

    return gaids

if __name__ == "__main__":
    # Check if verbose mode
    verbose = '-v' in sys.argv or '--verbose' in sys.argv
    
    try:
        gaids = main()
        # Success message only in verbose mode
        if verbose:
            print(f"\nSuccessfully processed {len(gaids)} subaccounts!")
    except Exception as e:
        # Only show errors in verbose mode or write to stderr
        if verbose:
            print(f"Error: {e}", file=sys.stderr)
            import traceback
            traceback.print_exc()
        else:
            # In non-verbose mode, output a valid JSON error
            print(json.dumps({"error": str(e)}), file=sys.stderr)
            sys.exit(1)
