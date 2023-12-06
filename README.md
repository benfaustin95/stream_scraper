# Stream Scraper
#### Ben Austin CS410P-005, Fall 2023, bena2@pdx.edu
***
## Project Information
### About
The purpose of my final project is to maintain a database of catalog streaming data grabbed from a combination
of Spotify's web api and scraped from Spotify's web player. Spotify has a fairly robust developer API for gaining artist, album, 
and track detail however they do not allow easy access to the actual play-count. I was looking through their web player and found that they
do actually send the updated play-count in their network response (but they do not display it anywhere). I found that if I intercepted the 
network traffic I could parse through it to get access to the play-count, then I could use that parsed information in 
conjunction with the information gained from the web api to populate the database, thus that is what I attempted to implement.
I did have hard time utilizing available crates (playwright, thiryfour(Selenium port)) to reliably intercept the network requests.
So I created three AWS lambda endpoints (in JS) to handle artist, album, and track scraping. The end point urls are in the .env file as 
I have to pay for each use once they reach a  certain monthly threshold of uses. However, I would be happy to share them (and my entire .env)
for grading purposes I just didn't want them publicly available. 

My project contains three primary components:
    - Library: Contains all the functionality of the program broke into 
    - Daily Update: The binary crate in control populating the database daily with updated information
    - Server: The binary crate in control of the server which handles getting information from the database

### Build & Run
**PLEASE NOTE running the program and tests requires .env information that is not in the repository.
If needed for grading purposes please reach out and I am more than happy to send it to you.**
- Run Server: cargo run --bin server
- Run Daily Update: cargo run --bin daily_update
- Run Tests: cargo test
### Testing
**PLEASE NOTE running the program and tests requires .env information that is not in the repository.
If needed for grading purposes please reach out and I am more than happy to send it to you.**
Testing such a wide scoped project was far more difficult than with the previous homework assignments as the majority of my
functionality depends on a live database. I also did not have the time to create a dummy database. For components interacting with
the database I primarily tested that the connection was able to be established and data was able to be successfully retrieved.
After testing the database reliant components I moved on to testing the structs and traits related to fetching data via HTTP
requests. Finally, I tested the components related to formatting and outputting data. If I could go back in time and re-allocate
my time I would have invested in setting up and seeding a dummy database upfront rather than trying to develop and work with live data.
I would also have incorporated hard coded data for testing the HTTP related components, as that would have allowed for more robust testing.
I pushed testing off until the end of the development process, when if I had implemented unit tests as I developed the process would
have resulted in a much more productive experience.
### Example
- [Presentation](./PRESENTATION.mp4)
***
## Development Experience
I had a really fun time implementing the project. For the first time I feel like I actually created something is useful
and I am really interested in. I have always had to go to other sources to get up-to-date streaming statistics, but 
now I have them at my figure tips whenever I would like. Although I did really enjoy the process it was not super smooth and 
I still have a lot to implement. I wasted a lot of time trying to get an automated browser tester to work in rust, so I had 
to pivot in order to have time to complete the implementation. Over winter break I want to work on implementing the actual
scraping portion in rust, improve the speed of the program (primarily through more efficient use of parallelism), and 
make the server router more robust. 
***
[License](./LICENSE.txt)
