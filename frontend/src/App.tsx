import { Route, Routes } from "react-router-dom";
import Admin from "./routes/Admin";
import Dashboard from "./routes/Dashboard";
import Docs from "./routes/Docs";

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<Dashboard />} />
      <Route path="/admin" element={<Admin />} />
      <Route path="/docs" element={<Docs />} />
    </Routes>
  );
}
