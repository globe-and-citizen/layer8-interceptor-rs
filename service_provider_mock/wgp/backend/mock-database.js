require("dotenv").config();
const BACKEND_URL = process.env.BACKEND_URL;

module.exports = {
    poems: [
            {
                "title": "The Red Wheelbarrow",
                "author": "WILLIAM CARLOS WILLIAMS",
                "body": "so much depends,\n upon \n a red wheel\nbarrow\nglazed with rain\nwater\nbeside the white\nchickens"
            },
            {
                "title": "We Real Cool",
                "author": "Gwendolyn Brooks",
                "body": "We real cool. We\nLeft school. We\nLurk late. We\nStrike straight. We\nSing sin. We\nThin gin. We\nJazz June. We\nDie soon."
            },
            {
                "title": "The Road Not Taken",
                "author": "ROBERT FROST",
                "body": "Two roads diverged in a yellow wood,\nAnd sorry I could not travel both\nAnd be one traveler, long I stood\nAnd looked down one as far as I could\nTo where it bent in the undergrowth;"
            }
        ],
    users: [
        {
            "email": "star",
            "password": "$2b$10$ssDWc4sXNoafqEdAsvH8TOUXywGsFHsPEODTZlSB4AKe8cqe1PmCi",
            "profile_image": `${BACKEND_URL}/media/girl.png`,
        },
        {
            "email": "tester",
            "password": "$2a$04$2N3n/C7AS1sRZLQApPkTN.CTTctruI716YzbGMoGAd0etIHzI42UW",
            "profile_image": `${BACKEND_URL}/media/boy.png`,
        },
        {
            "email": "osoro",
            "password":"$2a$04$WCLv2j8q0IN3.aKBwghme.oq74zwRXIN5E3Mg/NHGZCd.L6G37X9m",
            "profile_image": `${BACKEND_URL}/media/boy.png`,
        }
    ]
}