#!/usr/bin/env python3
"""
Script to list all existing subaccounts and their types to understand what types are available.
"""

import json
import sys
import green_gdk as gdk

def main():
    environment = sys.argv[1] if len(sys.argv) > 1 else 'amp'
    
    # Initialize GDK
    gdk.init({
        'datadir': '.',
        'log_level': 'warn'
    })
    
    mnemonics = {
        'dev': "brick jump above ten cargo hobby forum deer remove curve lion embrace ecology trim increase purchase menu curve prosper blame blanket combine color pelican",
        'test': "voice twelve rhythm cannon rebuild glove drift quiz spider rebuild cake eight abandon gauge frog animal cram peanut blossom pumpkin already scheme rookie physical",
        'amp': "blanket awful machine pudding soft feature toe panel primary biology salon remove aspect creek thank true ridge milk right father drive economy gold filter"
    }
    
    mnemonic = mnemonics[environment]
    session = gdk.Session({'name': 'testnet-liquid'})
    credentials = {'mnemonic': mnemonic}
    
    try:
        session.login_user({}, credentials).resolve()
        print(f"Logged in to {environment} environment")
    except:
        print(f"Could not login to {environment} environment")
        return
    
    # Get all subaccounts
    subaccounts_result = session.get_subaccounts().resolve()
    
    print("\nExisting subaccounts:")
    print("-" * 80)
    
    # Group by type
    by_type = {}
    for subaccount in subaccounts_result['subaccounts']:
        acc_type = subaccount.get('type', 'STANDARD')
        if acc_type not in by_type:
            by_type[acc_type] = []
        by_type[acc_type].append(subaccount)
    
    # Print by type
    for acc_type, accounts in by_type.items():
        print(f"\nType: {acc_type} ({len(accounts)} accounts)")
        if accounts:
            # Print first account details
            acc = accounts[0]
            print(f"  Example: {acc['name']}")
            print(f"  Available fields:")
            for key, value in acc.items():
                if key != 'name':
                    print(f"    {key}: {value}")

if __name__ == "__main__":
    main()
