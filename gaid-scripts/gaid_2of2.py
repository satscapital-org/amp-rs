#!/usr/bin/env python3
"""
Script to get addresses and public keys from existing 2of2 subaccounts on Liquid testnet.

This script uses the GDK (Green Development Kit) Python bindings to:
1. Create a session connected to Liquid testnet
2. Login with a test mnemonic
3. Use existing 2of2 subaccounts
4. Display all addresses and public keys

Usage:
    python gaid_2of2.py [environment]
    
    environment: 'dev', 'test', or 'amp' (defaults to 'dev')
"""

import json
import sys
import time
import green_gdk as gdk

def main():
    # Parse command line argument for environment
    valid_environments = ['dev', 'test', 'amp']
    environment = 'dev'  # default
    
    if len(sys.argv) > 1:
        environment = sys.argv[1].lower()
        if environment not in valid_environments:
            print(f"Error: Invalid environment '{environment}'")
            print(f"Valid environments are: {', '.join(valid_environments)}")
            sys.exit(1)
    
    print(f"Using environment: {environment}")
    
    # Initialize GDK
    gdk.init({
        'datadir': '.',     # Use current directory for any state files
        'log_level': 'warn'
    })
    
    # Environment-specific mnemonics - DO NOT use these for real funds!
    # These are deterministic test mnemonics for each environment
    mnemonics = {
        'dev': "brick jump above ten cargo hobby forum deer remove curve lion embrace ecology trim increase purchase menu curve prosper blame blanket combine color pelican",
        'test': "voice twelve rhythm cannon rebuild glove drift quiz spider rebuild cake eight abandon gauge frog animal cram peanut blossom pumpkin already scheme rookie physical",
        'amp': "blanket awful machine pudding soft feature toe panel primary biology salon remove aspect creek thank true ridge milk right father drive economy gold filter"
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
    def login_with_retry(session, credentials, max_retries=3):
        for attempt in range(max_retries):
            try:
                # Try to login first (in case wallet already exists)
                try:
                    print("Attempting to login to existing wallet...")
                    session.login_user({}, credentials).resolve()
                    print("Successfully logged in to existing wallet")
                    return session
                except RuntimeError as e:
                    if 'id_login_failed' in str(e):
                        # If login fails, register a new wallet
                        print("Wallet doesn't exist, registering new wallet...")
                        session.register_user({}, credentials).resolve()
                        session.login_user({}, credentials).resolve()
                        print("Successfully registered and logged in")
                        return session
                    else:
                        raise
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
    
    # Get existing subaccounts
    subaccounts_result = session.get_subaccounts().resolve()
    existing_subaccounts = {}
    
    # Build a map of existing subaccounts by name
    for subaccount in subaccounts_result['subaccounts']:
        # Use the existing 2of2 accounts that were already created
        if subaccount.get('type', '') == '2of2':
            existing_subaccounts[subaccount['name']] = subaccount
    
    # Get the first 20 2of2 subaccounts
    accounts = []
    print("\nRetrieving first 20 2of2 subaccounts...")
    
    # Get all 2of2 accounts
    all_2of2_accounts = []
    for subaccount in subaccounts_result['subaccounts']:
        if subaccount.get('type', '') == '2of2':
            all_2of2_accounts.append(subaccount)
    
    # Sort by pointer to get consistent ordering
    all_2of2_accounts.sort(key=lambda x: x['pointer'])
    
    # Take first 20
    for i, subaccount in enumerate(all_2of2_accounts[:20], 1):
        account_name = subaccount['name']
        pointer = subaccount['pointer']
        print(f"  Processing subaccount {i:2d}: {account_name if account_name else f'2of2 Account (pointer: {pointer})'}")
        subaccount_details = subaccount
        
        # Get an address for the subaccount
        address_details = None
        try:
            address_details = session.get_receive_address({'subaccount': pointer}).resolve()
            address = address_details.get('address', 'N/A')
        except Exception as e:
            print(f"    Warning: Could not get address for subaccount {i}: {e}")
            address = 'N/A'
        
        # Get service_xpub and script from address details
        service_xpub = 'N/A'
        script = 'N/A'
        user_pubkey = 'N/A'
        service_pubkey = 'N/A'
        
        if address_details:
            service_xpub = address_details.get('service_xpub', 'N/A')
            script = address_details.get('script', 'N/A')
            
            # Parse the script to extract public keys
            # The script format for 2of2 is:
            # OP_DEPTH OP_1SUB OP_IF <user_pubkey> OP_CHECKSIGVERIFY OP_ELSE OP_2 <user_pubkey> <service_pubkey> OP_2 OP_CHECKMULTISIG OP_ENDIF
            if script != 'N/A' and len(script) > 100:
                try:
                    # Skip initial opcodes (748c63) and get first pubkey length (21 = 33 bytes)
                    first_pubkey_start = 6  # After 748c63
                    if script[first_pubkey_start:first_pubkey_start+2] == '21':  # 33 bytes pubkey
                        user_pubkey = script[first_pubkey_start+2:first_pubkey_start+2+66]  # 33 bytes = 66 hex chars
                        
                        # Find second pubkey (after more opcodes)
                        # Look for the second 21 (33 bytes indicator) after position ~80
                        second_21_pos = script.find('21', 80)
                        if second_21_pos != -1:
                            service_pubkey = script[second_21_pos+2:second_21_pos+2+66]
                except Exception as e:
                    print(f"    Warning: Could not parse script: {e}")
        
        
        accounts.append({
            'index': i,
            'name': account_name if account_name else f'2of2 Account (pointer: {pointer})',
            'pointer': pointer,
            'address': address,
            'user_pubkey': user_pubkey,
            'service_pubkey': service_pubkey,
            'service_xpub': service_xpub
        })
    
    # Display all account details
    print("\n" + "=" * 100)
    print(f"2of2 Subaccounts with Addresses and Public Keys (Environment: {environment.upper()}):")
    print("=" * 100)
    
    for account in accounts:
        print(f"{account['index']:2d}. {account['name']}")
        print(f"    Address: {account['address']}")
        print(f"    User PubKey: {account['user_pubkey']}")
        print(f"    Service PubKey: {account['service_pubkey']}")
        print(f"    Service XPub: {account['service_xpub']}")
        print()
    
    print("=" * 100)
    print(f"Total subaccounts: {len(accounts)}")
    print("=" * 100)
    
    return accounts

if __name__ == "__main__":
    try:
        accounts = main()
        print(f"\nSuccessfully processed {len(accounts)} subaccounts!")
    except Exception as e:
        print(f"\nError: {e}")
        import traceback
        traceback.print_exc()
