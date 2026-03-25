bin_path="/home/aaron/Documents/rust/worst/target/debug/worst-switcher"
result=$("$bin_path" $*)
if [ -n "$result" ] && [ -z "${result##EXEC::*}" ]; then
   eval "${result#EXEC::}"
else
   echo "$result" 
fi