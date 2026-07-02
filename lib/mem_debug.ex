defmodule Yex.MemDebug do
  require Yex.Doc

  @moduledoc """
  Native (Rust NIF) memory debugging counters for y_ex.

  Counters track hot paths in the NIF layer: doc creation, subdoc map integration,
  YDoc reads, update encoding, subscriptions, etc.

  Build y_ex from source when using this module:

      RUSTLER_PRECOMPILATION_YEX_BUILD=true mix deps.compile y_ex --force

  Or set the same env var when starting the application.
  """

  @type snapshot :: %{
          enabled: boolean(),
          doc_new: non_neg_integer(),
          doc_with_options: non_neg_integer(),
          map_set_doc: non_neg_integer(),
          map_set_null: non_neg_integer(),
          map_set_other: non_neg_integer(),
          yout_ydoc_wrap: non_neg_integer(),
          monitor_update_v1: non_neg_integer(),
          monitor_update_v2: non_neg_integer(),
          monitor_subdocs: non_neg_integer(),
          sub_unsubscribe: non_neg_integer(),
          encode_state_as_update: non_neg_integer(),
          apply_update: non_neg_integer(),
          transaction_begin: non_neg_integer(),
          transaction_commit: non_neg_integer(),
          net_subdoc_integrations: integer()
        }

  @type doc_info :: %{
          guid: String.t(),
          client_id: non_neg_integer(),
          skip_gc: boolean(),
          auto_load: boolean(),
          state_vector_bytes: non_neg_integer(),
          update_encode_bytes: non_neg_integer(),
          has_worker_pid: boolean()
        }

  @doc "Enable or disable native counter recording."
  @spec enable(boolean()) :: :ok
  def enable(enabled), do: Yex.Nif.mem_debug_enable(enabled)

  @doc "Reset all native counters to zero."
  @spec reset() :: :ok
  def reset, do: Yex.Nif.mem_debug_reset()

  @doc "Return current native counter snapshot."
  @spec snapshot() :: snapshot()
  def snapshot, do: Yex.Nif.mem_debug_snapshot()

  @doc "Inspect yrs store size metrics for a document."
  @spec inspect_doc(Yex.Doc.t()) :: doc_info()
  def inspect_doc(%Yex.Doc{} = doc) do
    Yex.Doc.run_in_worker_process(doc, do: Yex.Nif.mem_debug_inspect_doc(doc))
  end

  @doc """
  Append an NDJSON line with native snapshot to `path`.
  Optional `extra` map is merged into the JSON payload.
  """
  @spec log(String.t(), String.t(), map()) :: :ok
  def log(path, phase, extra \\ %{}) do
    extra_json =
      case Jason.encode(extra) do
        {:ok, json} -> json
        _ -> "{}"
      end

    Yex.Nif.mem_debug_log(path, phase, extra_json)
  end

  @doc "Encode full document update and return `{:ok, byte_size, binary}`."
  @spec encode_update_size(Yex.Doc.t()) :: {:ok, non_neg_integer(), binary()}
  def encode_update_size(%Yex.Doc{} = doc) do
    Yex.Doc.run_in_worker_process doc do
      case Yex.Nif.mem_debug_encode_update_size(doc) do
        {:ok, size, bin} -> {:ok, size, bin}
        other -> other
      end
    end
  end

  @doc "Return snapshot fields as a plain map (snake_case keys)."
  @spec snapshot_map() :: map()
  def snapshot_map do
    snapshot()
    |> Map.from_struct()
  end
end
