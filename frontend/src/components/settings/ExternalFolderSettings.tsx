"use client";

import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";
import {
  useExternalFolderSettings,
  useConnectExternalFolder,
  useUpdateExternalFolder,
} from "@/hooks/useExternalFolder";
import { Button } from "@/components/ui/button";

export function ExternalFolderSettings() {
  const { data, isLoading } = useExternalFolderSettings();
  const connect = useConnectExternalFolder();
  const update = useUpdateExternalFolder();
  const [usuario, setUsuario] = useState("");
  const [senha, setSenha] = useState("");
  const [baseUrl, setBaseUrl] = useState("");

  const folderName = data?.folder_name ?? "External";

  useEffect(() => {
    if (data) setBaseUrl(data.base_url ?? "");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data?.base_url]);

  const handleConnect = () => {
    if (!usuario.trim() || !senha) {
      toast.error("Informe usuário e senha.");
      return;
    }
    connect.mutate(
      {
        usuario: usuario.trim(),
        senha,
        base_url: baseUrl.trim() || null,
      },
      {
        onSuccess: () => {
          setSenha("");
          toast.success("Conta conectada.");
        },
        onError: (e) => toast.error(e.message),
      },
    );
  };

  const handleToggle = (enabled: boolean) => {
    update.mutate(
      { enabled },
      { onError: (e) => toast.error(e.message) },
    );
  };

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        Loading...
      </div>
    );
  }

  if (!data) return null;

  return (
    <div className="max-w-2xl space-y-6">
      <div>
        <h2 className="text-base font-semibold">{folderName}</h2>
        <p className="mt-0.5 text-sm text-muted-foreground">
          Conecte uma conta para mostrar os itens desta pasta no menu lateral.
        </p>
      </div>

      {!data.configured && (
        <p className="rounded-lg border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
          Este recurso não está configurado no servidor.
        </p>
      )}

      {data.configured && !data.available && (
        <p className="rounded-lg border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
          O domínio do seu e-mail não está habilitado para este recurso.
        </p>
      )}

      {data.available && (
        <>
          {data.connected && (
            <div className="flex items-center justify-between rounded-lg border border-border p-4">
              <div>
                <div className="text-sm font-medium">Ativar pasta</div>
                <p className="mt-0.5 text-xs text-muted-foreground">
                  {data.account_email
                    ? `Conectado como ${data.account_email}`
                    : "Conta conectada"}
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={data.enabled}
                disabled={update.isPending}
                onClick={() => handleToggle(!data.enabled)}
                className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors disabled:opacity-50 ${
                  data.enabled ? "bg-primary" : "bg-muted"
                }`}
              >
                <span
                  className={`inline-block size-4 transform rounded-full bg-background shadow transition-transform ${
                    data.enabled ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
          )}

          <div className="space-y-4 rounded-lg border border-border p-4">
            <div>
              <div className="text-sm font-medium">
                {data.connected ? "Reconectar conta" : "Conectar conta"}
              </div>
              <p className="mt-0.5 text-xs text-muted-foreground">
                Use as credenciais de administrador do sistema externo.
              </p>
            </div>

            <div>
              <label htmlFor="external-usuario" className="text-sm font-medium">
                Usuário
              </label>
              <input
                id="external-usuario"
                type="text"
                autoComplete="username"
                value={usuario}
                onChange={(e) => setUsuario(e.target.value)}
                className="mt-1 w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
              />
            </div>

            <div>
              <label htmlFor="external-senha" className="text-sm font-medium">
                Senha
              </label>
              <input
                id="external-senha"
                type="password"
                autoComplete="current-password"
                value={senha}
                onChange={(e) => setSenha(e.target.value)}
                className="mt-1 w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
              />
            </div>

            <div>
              <label htmlFor="external-base-url" className="text-sm font-medium">
                URL base das imagens
              </label>
              <input
                id="external-base-url"
                type="url"
                inputMode="url"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
                placeholder="https://exemplo.com/ARQUIVOS/THUMBS/"
                className="mt-1 w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
              />
              <p className="mt-1 text-xs text-muted-foreground">
                Opcional. O nome da imagem é anexado a esta URL para exibir a miniatura.
              </p>
            </div>

            <Button
              type="button"
              onClick={handleConnect}
              disabled={connect.isPending}
            >
              {connect.isPending && <Loader2 className="size-4 animate-spin" />}
              Conectar
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
