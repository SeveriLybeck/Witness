import { useState, useContext, memo, createContext } from "react";

const Theme = createContext("light");

export function Counter({ label, step }: { label: string; step: number }) {
  const [count, setCount] = useState(0);
  const theme = useContext(Theme);
  return <button className={theme} onClick={() => setCount(count + step)}>{label}: {count}</button>;
}

export const Badge = memo(function Badge(props: { text: string }) {
  return <span>{props.text}</span>;
});
