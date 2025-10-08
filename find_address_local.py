#!/usr/bin/env python3
"""
Script to find which GAID owns a specific address using the local mnemonics.
"""

import json
import sys
import os
import green_gdk as gdk

def check_address_in_environment(environment, address, verbose=False):
    """Check all GAIDs in an environment for the given address."""
    
    # Suppress warnings if not verbose
    original_stderr = None
    if not verbose:
        original_stderr = sys.stderr
        sys.stderr = open(os.devnull, 'w')
    
    try:
        gdk.init({'datadir': '.', 'log_level': 'warn'})
    except:
        pass
    
    # Environment-specific mnemonics (from local gaid.py)
    mnemonics = {
        'dev': "vote ball voice juice visit license music off paddle hold suffer beach someone hover wheat boring protect broccoli issue opinion uniform media update arrange",
        'test': "hood novel finish couch rack pistol march army gun bring universe unknown hurry famous vote enact inspire whisper violin blur chief into engage cigar",
        'amp': "perfect grit monkey disorder cliff exhibit meat giant dream secret liberty walnut later caught snow wasp bulb bind feature couple vault flower credit bundle"
    }
    
    mnemonic = mnemonics.get(environment)
    if not mnemonic:
        return None
    
    # Create session
    session = gdk.Session({'name': 'testnet-liquid'})
    credentials = {'mnemonic': mnemonic}
    
    try:
        session.login_user({}, credentials).resolve()
    except:
        try:
            session.register_user({}, credentials).resolve()
            session.login_user({}, credentials).resolve()
        except:
            return None
    
    # Get all subaccounts
    subaccounts_result = session.get_subaccounts().resolve()
    
    # Check each 2of2_no_recovery subaccount
    for subaccount in subaccounts_result['subaccounts']:
        if subaccount['type'] != '2of2_no_recovery':
            continue
            
        gaid = subaccount['receiving_id']
        pointer = subaccount['pointer']
        
        if verbose:
            print(f"  Checking GAID: {gaid}", end=" ... ")
        
        # Check current receive address
        try:
            current_addr = session.get_receive_address({'subaccount': pointer}).resolve()
            if current_addr['address'] == address:
                if verbose:
                    print("FOUND! (current receive address)")
                else:
                    if original_stderr:
                        sys.stderr.close()
                        sys.stderr = original_stderr
                return gaid, environment, "current"
        except:
            pass
        
        # Check previous addresses
        try:
            prev_addrs = session.get_previous_addresses({
                'subaccount': pointer,
                'last_pointer': 100  # Check first 100 addresses
            }).resolve()
            
            if 'list' in prev_addrs:
                for i, addr_info in enumerate(prev_addrs['list']):
                    if addr_info.get('address') == address:
                        if verbose:
                            print(f"FOUND! (address index {i})")
                        else:
                            if original_stderr:
                                sys.stderr.close()
                                sys.stderr = original_stderr
                        return gaid, environment, f"index_{i}"
        except:
            pass
        
        if verbose:
            print("not found")
    
    if not verbose and original_stderr:
        sys.stderr.close()
        sys.stderr = original_stderr
    
    return None

def main():
    if len(sys.argv) < 2:
        print("Usage: python find_address_local.py <address> [-v|--verbose]")
        sys.exit(1)
    
    address = sys.argv[1]
    verbose = '-v' in sys.argv or '--verbose' in sys.argv
    
    print(f"Searching for address: {address}")
    if verbose:
        print()
    
    environments = ['dev', 'test', 'amp']
    found = False
    
    for env in environments:
        if verbose:
            print(f"Checking {env} environment:")
        
        result = check_address_in_environment(env, address, verbose)
        
        if result:
            gaid, environment, location = result
            found = True
            print(f"\n✓ Address found!")
            print(f"  Environment: {environment}")
            print(f"  GAID: {gaid}")
            print(f"  Location: {location}")
            
            # Now check balance for this GAID
            print(f"\nChecking balance for {gaid} in {environment} environment...")
            os.system(f"python3 balance.py {environment} {gaid}")
            break
        elif verbose:
            print(f"  Not found in {env} environment\n")
    
    if not found:
        print(f"\n✗ Address not found in any environment")

if __name__ == "__main__":
    main()