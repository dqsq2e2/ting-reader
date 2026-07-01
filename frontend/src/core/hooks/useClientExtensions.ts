import { useCallback, useEffect, useMemo, useState } from "react";
import { listPluginCapabilities } from "../api/pluginCapabilities";
import {
  buildClientExtensionRegistry,
  type ClientExtensionRegistrySnapshot,
} from "../pluginExtensions";

type ClientExtensionState = {
  loading: boolean;
  error?: string;
  registry: ClientExtensionRegistrySnapshot;
  refresh: () => Promise<void>;
};

const emptyRegistry: ClientExtensionRegistrySnapshot = {
  extensions: [],
  bySlot: {},
};

export const useClientExtensions = (): ClientExtensionState => {
  const [registrations, setRegistrations] = useState<
    Awaited<ReturnType<typeof listPluginCapabilities>>
  >([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(undefined);
    try {
      const [uiExtensions, clientExtensions] = await Promise.all([
        listPluginCapabilities("ui_extension"),
        listPluginCapabilities("client_extension"),
      ]);
      setRegistrations([...uiExtensions, ...clientExtensions]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setRegistrations([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const registry = useMemo(
    () =>
      registrations.length > 0
        ? buildClientExtensionRegistry(registrations)
        : emptyRegistry,
    [registrations],
  );

  return { loading, error, registry, refresh };
};
