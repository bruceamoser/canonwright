import { compareCanonTitles, type BookView, type CanonLink } from "./studio";

type Entry = BookView["book"]["records"][number];

export function LinkEditor({ label, value, entries, onChange }: {
  label: string; value: CanonLink[]; entries: Entry[]; onChange: (links: CanonLink[]) => void;
}) {
  function update(index: number, changes: Partial<CanonLink>) {
    onChange(value.map((link, i) => i === index ? { ...link, ...changes } : link));
  }
  return <fieldset className="studio-link-editor"><legend>{label}</legend>
    {value.map((link, i) => <div key={i}>
      <select aria-label={`${label} target ${i + 1}`} value={link.target_id ?? ""} onChange={e => {
        const target = entries.find(entry => entry.id === e.target.value);
        update(i, { target_id: target?.id, name: target?.finding.title ?? link.name });
      }}>
        <option value="">Unresolved name</option>
        {entries.filter(entry => entry.finding.kind === "entry").sort(compareCanonTitles).map(entry => <option key={entry.id} value={entry.id}>
          {entry.finding.title} · {entry.finding.area}{entry.pending ? " · In review" : ""}
        </option>)}
      </select>
      {!link.target_id && <input aria-label="Name in manuscript" placeholder="Name in manuscript" required value={link.name} onChange={e => update(i, {name: e.target.value})} />}
      <input aria-label="Connection" placeholder="How are they connected?" required value={link.context} onChange={e => update(i, {context: e.target.value})} />
      <button type="button" onClick={() => onChange(value.filter((_, n) => n !== i))}>Remove link</button>
    </div>)}
    <button type="button" onClick={() => onChange([...value, { name: "", context: "" }])}>Add link</button>
  </fieldset>;
}

export function RecordLinks({record, entries, open}: {record: Entry; entries: Entry[]; open: (id: string) => void}) {
  const incoming = entries.flatMap(entry => [...Object.values(entry.finding.fields), ...(entry.retained ?? []).map(field => field.value)].flatMap(value =>
    Array.isArray(value) ? value.flatMap(link => typeof link !== "string" && link.target_id === record.id ? [{entry, link}] : []) : []));
  // Older entries contain prose only. Link literal, unambiguous names separately from authored relationships.
  const tokens = (text: string) => text.toLowerCase().match(/[\p{L}\p{N}]+/gu)?.join(" ") ?? "";
  const summary = ` ${tokens(record.finding.detail)} `;
  const names = new Map<string, Entry | null>();
  for (const entry of entries) {
    if (entry.finding.kind !== "entry") continue;
    const name = tokens(entry.finding.title);
    const prior = names.get(name);
    if (prior === undefined) names.set(name, entry);
    else if (prior && prior.pending !== entry.pending && prior.section === entry.section &&
      prior.finding.area === entry.finding.area && Boolean(prior.finding.evidence) &&
      prior.finding.evidence === entry.finding.evidence) {
      names.set(name, prior.pending ? entry : prior);
    } else names.set(name, null);
  }
  const mentions = Object.keys(record.finding.fields).length ? [] : Array.from(names).flatMap(([name, entry]) =>
    entry && entry.id !== record.id && name !== tokens(record.finding.title) && name && summary.includes(` ${name} `) ? [entry] : []);
  return <>
    {Boolean(record.retained?.length) && <details><summary>Retained fields</summary>
      {record.retained?.map((field, index) => <section key={index}><h4>{field.label}</h4>
        {typeof field.value === "string" ? field.value : <ul>{field.value.map((item, i) => <li key={i}>
          {typeof item === "string" ? item : <>{item.target_id && entries.some(entry => entry.id === item.target_id)
            ? <button type="button" className="studio-canon-link" onClick={() => open(item.target_id!)}>{item.name}</button>
            : item.name} · {item.context}</>}
        </li>)}</ul>}</section>)}
    </details>}
    {mentions.length > 0 && <section><h4>Names in summary</h4><ul>{mentions.map(entry =>
      <li key={entry.id}><button type="button" className="studio-canon-link" onClick={() => open(entry.id)}>{entry.finding.title}</button>
      {entry.pending && " (in review)"}</li>)}</ul></section>}
    {incoming.length > 0 && <section><h4>Referenced by</h4><ul>{incoming.map(({entry, link}, i) => <li key={i}>
    <button type="button" className="studio-canon-link" onClick={() => open(entry.id)}>{entry.finding.title}</button> · {link.context}{entry.pending && " (in review)"}
  </li>)}</ul></section>}</>;
}
