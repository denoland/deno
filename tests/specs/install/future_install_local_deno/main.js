import { setValue } from "@denotest/esm-basic";
import { add } from "@denotest/add";
import { returnsHi } from "test-http";
setValue(5);
returnsHi();
add(2, 2);
