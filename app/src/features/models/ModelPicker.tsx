import { useEffect } from "react";
import { choiceKey, parseChoiceKey, priceLabel } from "./model";
import { useModels } from "./store";

/** The model menu above the message box (M7 §4). */
export function ModelPicker({ disabled }: { disabled: boolean }) {
  const { catalog, choice, loadCatalog, choose } = useModels();

  useEffect(() => {
    if (!catalog) void loadCatalog();
  }, [catalog, loadCatalog]);

  const current =
    catalog?.current_provider && catalog.current_model
      ? `${catalog.current_model}`
      : "Hermes's choice";

  return (
    <label className="model-picker">
      <span className="muted small">Model</span>
      <select
        value={choiceKey(choice)}
        disabled={disabled || !catalog}
        onChange={(e) => choose(parseChoiceKey(e.target.value))}
        title="The model for this conversation"
      >
        <option value="">Hermes default ({current})</option>
        {catalog?.providers.map((provider) => (
          <optgroup key={provider.slug} label={provider.name}>
            {provider.models.map((m) => {
              const price = priceLabel(m.price);
              return (
                <option key={m.id} value={choiceKey({ provider: provider.slug, model: m.id })}>
                  {m.id}
                  {price ? `, ${price}` : ""}
                </option>
              );
            })}
          </optgroup>
        ))}
      </select>
    </label>
  );
}
