import React, { createContext, useState, useContext, useEffect, ReactNode } from 'react';
import demoApiService from './demoApi';
import observabilityApi from '../api/observabilityApi';

// Create a context for the demo mode
interface DemoModeContextProps {
  isDemoMode: boolean;
  toggleDemoMode: (active: boolean) => void;
  api: typeof observabilityApi | typeof demoApiService;
}

const DemoModeContext = createContext<DemoModeContextProps>({
  isDemoMode: false,
  toggleDemoMode: () => {},
  api: observabilityApi,
});

// Create a provider component
interface DemoModeProviderProps {
  children: ReactNode;
}

export const DemoModeProvider: React.FC<DemoModeProviderProps> = ({ children }) => {
  // Check if demo mode was previously enabled (persisted in localStorage)
  const [isDemoMode, setIsDemoMode] = useState(() => {
    const savedMode = localStorage.getItem('icn-demo-mode');
    return savedMode === 'true';
  });

  // Determine which API to use based on demo mode
  const api = isDemoMode ? demoApiService : observabilityApi;

  // Toggle demo mode
  const toggleDemoMode = (active: boolean) => {
    setIsDemoMode(active);
    localStorage.setItem('icn-demo-mode', active.toString());
  };

  // Log when demo mode changes
  useEffect(() => {
    console.log(`Demo mode is ${isDemoMode ? 'enabled' : 'disabled'}`);
  }, [isDemoMode]);

  return (
    <DemoModeContext.Provider value={{ isDemoMode, toggleDemoMode, api }}>
      {children}
    </DemoModeContext.Provider>
  );
};

// Create a custom hook to use the demo mode context
export const useDemoMode = () => useContext(DemoModeContext);

export default DemoModeContext; 