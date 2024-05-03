// this directory import was not working (it should resolve via the package.json)
import { PreactContext } from '../..';

export declare function useContext<T>(context: PreactContext<T>): T;
