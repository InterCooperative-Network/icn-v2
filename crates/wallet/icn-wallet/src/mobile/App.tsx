import React from 'react';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { WalletScreen } from './wallet_screen';

export default function App() {
  return (
    <SafeAreaProvider>
      <WalletScreen />
    </SafeAreaProvider>
  );
} 