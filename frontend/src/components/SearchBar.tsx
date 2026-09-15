import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../api";
import type { SearchResult } from "../api";
import { searchResultHref, truncateMiddle } from "../format";

const TYPE_LABEL: Record<SearchResult["result_type"], string> = {
  pool: "Pool",
  token: "Token",
  account: "Account",
};

/**
 * Sidebar search across pools, tokens, and whale-payment accounts (backed
 * by `GET /search`). Debounces input locally rather than adding a shared
 * hook for one caller; the backend itself also refuses to search below two
 * characters (see `logic::search_like_pattern`), so this just avoids firing
 * a request per keystroke on top of that.
 */
export function SearchBar() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [loading, setLoading] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();

  const trimmed = query.trim();
  const searchable = trimmed.length >= 2;

  useEffect(() => {
    if (!searchable) return;
    const timer = setTimeout(() => {
      setLoading(true);
      api
        .search(trimmed)
        .then((rows) => {
          setResults(rows);
          setActiveIndex(-1);
        })
        .catch(() => setResults([]))
        .finally(() => setLoading(false));
    }, 250);
    return () => clearTimeout(timer);
  }, [trimmed, searchable]);

  const displayResults = searchable ? results : [];
  const displayLoading = searchable && loading;

  useEffect(() => {
    function onClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, []);

  function go(result: SearchResult) {
    navigate(searchResultHref(result));
    setQuery("");
    setResults([]);
    setOpen(false);
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (!open || displayResults.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => (i + 1) % displayResults.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => (i <= 0 ? displayResults.length - 1 : i - 1));
    } else if (e.key === "Enter") {
      const target = displayResults[activeIndex] ?? displayResults[0];
      if (target) go(target);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  }

  const showDropdown = open && searchable;

  return (
    <div className="search-bar" ref={containerRef}>
      <input
        type="search"
        className="search-input"
        placeholder="Search pools, tokens, accounts…"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={onKeyDown}
        aria-label="Global search"
      />
      {showDropdown && (
        <div className="search-results" role="listbox">
          {displayLoading && <div className="search-empty">Searching…</div>}
          {!displayLoading && displayResults.length === 0 && (
            <div className="search-empty">No matches for &ldquo;{trimmed}&rdquo;</div>
          )}
          {!displayLoading &&
            displayResults.map((r, i) => (
              <button
                type="button"
                key={`${r.result_type}:${r.key}`}
                className={"search-result" + (i === activeIndex ? " active" : "")}
                role="option"
                aria-selected={i === activeIndex}
                onMouseEnter={() => setActiveIndex(i)}
                onClick={() => go(r)}
              >
                <span className="search-result-type">{TYPE_LABEL[r.result_type]}</span>
                <span className="search-result-title">{truncateMiddle(r.title, 14, 8)}</span>
                {r.subtitle && (
                  <span className="search-result-subtitle">{truncateMiddle(r.subtitle, 8, 6)}</span>
                )}
              </button>
            ))}
        </div>
      )}
    </div>
  );
}
